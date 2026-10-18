#define _GNU_SOURCE
#include <errno.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <tinyalsa/pcm.h>
#include <unistd.h>

int pcm_avail_update(struct pcm *pcm);

enum {
    CAPTURE_CARD = 0,
    CAPTURE_DEVICE = 0,
    CAPTURE_RATE = 16000,
    CAPTURE_CHANNELS = 1,
    CAPTURE_SECONDS = 1,
    CAPTURE_FRAMES = CAPTURE_RATE * CAPTURE_SECONDS,
    CAPTURE_BYTES = CAPTURE_FRAMES * CAPTURE_CHANNELS * sizeof(int16_t),
    PERIOD_FRAMES = 320,
    PERIOD_COUNT = 4,
    HARD_TIMEOUT_SECONDS = 4,
};

static void zero_memory(void *ptr, size_t len)
{
    volatile unsigned char *p = ptr;
    while (len-- != 0)
        *p++ = 0;
}

static uint64_t integer_sqrt(uint64_t value)
{
    uint64_t bit = UINT64_C(1) << 62;
    uint64_t result = 0;
    while (bit > value)
        bit >>= 2;
    while (bit != 0) {
        if (value >= result + bit) {
            value -= result + bit;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }
    return result;
}

static volatile sig_atomic_t capture_timed_out;

static void bounded_timeout_handler(int signal_number)
{
    (void)signal_number;
    capture_timed_out = 1;
}

static int self_test(void)
{
    unsigned char bytes[8];
    memset(bytes, 0xa5, sizeof(bytes));
    zero_memory(bytes, sizeof(bytes));
    for (size_t i = 0; i < sizeof(bytes); ++i)
        if (bytes[i] != 0)
            return EXIT_FAILURE;
    if (integer_sqrt(0) != 0 || integer_sqrt(1) != 1 ||
        integer_sqrt(15) != 3 || integer_sqrt(16) != 4)
        return EXIT_FAILURE;
    puts("self_test=passed");
    return EXIT_SUCCESS;
}

static volatile sig_atomic_t stream_interrupted = 0;

static void stream_signal_handler(int sig)
{
    (void)sig;
    stream_interrupted = 1;
}

static int stream_audio(void)
{
    enum {
        STREAM_PERIOD_FRAMES = 320,
        STREAM_PERIOD_COUNT = 10,
        STREAM_CHUNK_FRAMES = 1600,
        STREAM_CHUNK_BYTES = STREAM_CHUNK_FRAMES * sizeof(int16_t),
    };
    static const struct pcm_config stream_config = {
        .channels = CAPTURE_CHANNELS,
        .rate = CAPTURE_RATE,
        .period_size = STREAM_PERIOD_FRAMES,
        .period_count = STREAM_PERIOD_COUNT,
        .format = PCM_FORMAT_S16_LE,
        .start_threshold = 1,
        .stop_threshold = STREAM_PERIOD_FRAMES * STREAM_PERIOD_COUNT,
        .silence_threshold = 0,
        .silence_size = 0,
        .avail_min = STREAM_PERIOD_FRAMES,
    };
    struct sigaction sa = {0};
    sa.sa_handler = stream_signal_handler;
    sigemptyset(&sa.sa_mask);
    sigaction(SIGTERM, &sa, NULL);
    sigaction(SIGINT, &sa, NULL);
    sigaction(SIGPIPE, &sa, NULL);

    int16_t *buf = calloc(1, STREAM_CHUNK_BYTES);
    if (buf == NULL) {
        perror("calloc");
        return EXIT_FAILURE;
    }
    (void)mlock(buf, STREAM_CHUNK_BYTES);

    struct pcm *pcm = pcm_open(CAPTURE_CARD, CAPTURE_DEVICE, PCM_IN, &stream_config);
    if (pcm == NULL) {
        fputs("pcm_open_failed\n", stderr);
        zero_memory(buf, STREAM_CHUNK_BYTES);
        free(buf);
        return EXIT_FAILURE;
    }
    if (!pcm_is_ready(pcm)) {
        fprintf(stderr, "pcm_not_ready=%s\n", pcm_get_error(pcm));
        pcm_close(pcm);
        zero_memory(buf, STREAM_CHUNK_BYTES);
        free(buf);
        return EXIT_FAILURE;
    }

    unsigned char ready = 0;
    if (write(STDOUT_FILENO, &ready, 1) != 1) {
        pcm_close(pcm);
        zero_memory(buf, STREAM_CHUNK_BYTES);
        free(buf);
        return EXIT_FAILURE;
    }

    while (!stream_interrupted) {
        (void)pcm_avail_update(pcm);
        int n = pcm_readi(pcm, buf, STREAM_CHUNK_FRAMES);
        if (n != STREAM_CHUNK_FRAMES) {
            if (n < 0) {
                pcm_prepare(pcm);
            }
            continue;
        }

        size_t written = 0;
        while (written < STREAM_CHUNK_BYTES && !stream_interrupted) {
            ssize_t nw = write(STDOUT_FILENO, ((unsigned char *)buf) + written, STREAM_CHUNK_BYTES - written);
            if (nw <= 0) {
                if (nw < 0 && (errno == EINTR || errno == EAGAIN)) {
                    continue;
                }
                stream_interrupted = 1;
                break;
            }
            written += (size_t)nw;
        }
    }

    pcm_close(pcm);
    zero_memory(buf, STREAM_CHUNK_BYTES);
    (void)munlock(buf, STREAM_CHUNK_BYTES);
    free(buf);
    return EXIT_SUCCESS;
}

int main(int argc, char **argv)
{
    static const struct pcm_config config = {
        .channels = CAPTURE_CHANNELS,
        .rate = CAPTURE_RATE,
        .period_size = PERIOD_FRAMES,
        .period_count = PERIOD_COUNT,
        .format = PCM_FORMAT_S16_LE,
        .start_threshold = 1,
        .stop_threshold = PERIOD_FRAMES * PERIOD_COUNT,
        .silence_threshold = 0,
        .silence_size = 0,
        .avail_min = PERIOD_FRAMES,
    };
    struct sigaction action = {0};
    struct pcm *pcm = NULL;
    int16_t *samples = NULL;
    uint64_t sum_squares = 0;
    uint64_t sum_absolute = 0;
    uint32_t clipped = 0;
    uint32_t zeros = 0;
    uint32_t peak = 0;
    int result = EXIT_FAILURE;

    if (argc == 2 && strcmp(argv[1], "--self-test") == 0)
        return self_test();
    if (argc == 2 && (strcmp(argv[1], "stream") == 0 || strcmp(argv[1], "--stream") == 0))
        return stream_audio();
    if (argc != 1) {
        fputs("usage: cleankvm-audio-capture-probe [--self-test|stream]\n", stderr);
        return EXIT_FAILURE;
    }

    action.sa_handler = bounded_timeout_handler;
    sigemptyset(&action.sa_mask);
    if (sigaction(SIGALRM, &action, NULL) != 0) {
        perror("sigaction");
        return EXIT_FAILURE;
    }
    alarm(HARD_TIMEOUT_SECONDS);

    samples = calloc(1, CAPTURE_BYTES);
    if (samples == NULL) {
        perror("calloc");
        goto cleanup;
    }
    if (mlock(samples, CAPTURE_BYTES) != 0) {
        perror("mlock");
        goto cleanup;
    }

    pcm = pcm_open(CAPTURE_CARD, CAPTURE_DEVICE, PCM_IN, &config);
    if (pcm == NULL) {
        fputs("pcm_open_allocation_failed\n", stderr);
        goto cleanup;
    }
    if (!pcm_is_ready(pcm)) {
        fprintf(stderr, "pcm_not_ready=%s\n", pcm_get_error(pcm));
        goto cleanup;
    }
    if (pcm_readi(pcm, samples, CAPTURE_FRAMES) != CAPTURE_FRAMES) {
        fprintf(stderr, "pcm_read_failed=%s\n", pcm_get_error(pcm));
        goto cleanup;
    }
    pcm_close(pcm);
    pcm = NULL;

    for (uint32_t i = 0; i < CAPTURE_FRAMES; ++i) {
        int32_t value = samples[i];
        uint32_t absolute = value < 0 ? (uint32_t)(-value) : (uint32_t)value;
        if (absolute > peak)
            peak = absolute;
        if (value == INT16_MIN || value == INT16_MAX)
            ++clipped;
        if (value == 0)
            ++zeros;
        sum_absolute += absolute;
        sum_squares += (uint64_t)((int64_t)value * value);
    }

    printf("capture_frames=%u\n", CAPTURE_FRAMES);
    printf("capture_bytes=%u\n", CAPTURE_BYTES);
    printf("rate_hz=%u\n", CAPTURE_RATE);
    printf("channels=%u\n", CAPTURE_CHANNELS);
    printf("format=s16_le\n");
    printf("peak_abs=%u\n", peak);
    printf("mean_abs=%llu\n", (unsigned long long)(sum_absolute / CAPTURE_FRAMES));
    printf("rms=%llu\n", (unsigned long long)integer_sqrt(sum_squares / CAPTURE_FRAMES));
    printf("clipped_samples=%u\n", clipped);
    printf("zero_samples=%u\n", zeros);
    result = EXIT_SUCCESS;

cleanup:
    if (pcm != NULL)
        pcm_close(pcm);
    if (samples != NULL) {
        zero_memory(samples, CAPTURE_BYTES);
        (void)munlock(samples, CAPTURE_BYTES);
        free(samples);
    }
    alarm(0);
    if (capture_timed_out) {
        fputs("capture_timeout\n", stderr);
        return 124;
    }
    return result;
}
