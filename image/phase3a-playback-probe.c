#define _GNU_SOURCE
#include <errno.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <tinyalsa/mixer.h>
#include <tinyalsa/pcm.h>
#include <sound/asound.h>
#include <unistd.h>

enum {
    PLAYBACK_CARD = 1,
    PLAYBACK_DEVICE = 0,
    PLAYBACK_RATE = 16000,
    PLAYBACK_CHANNELS = 1,
    PLAYBACK_FRAMES = PLAYBACK_RATE,
    PLAYBACK_BYTES = PLAYBACK_FRAMES * sizeof(int16_t),
    PERIOD_FRAMES = 320,
    PERIOD_COUNT = 4,
    HARD_TIMEOUT_SECONDS = 4,
    /* The inverted ALSA control maps 32 to hardware zero and 31 to the
     * driver's lowest nonzero DAC gain code. */
    SAFE_MUTED_VOLUME = 32,
    TEST_VOLUME = 31,
    TONE_PEAK = 512,
};

static volatile sig_atomic_t playback_timed_out;

static void zero_memory(void *ptr, size_t len)
{
    volatile unsigned char *p = ptr;
    while (len-- != 0)
        *p++ = 0;
}

static void bounded_timeout_handler(int signal_number)
{
    (void)signal_number;
    playback_timed_out = 1;
}

static int set_stereo_control(struct mixer_ctl *control, int value)
{
    unsigned int count;

    if (control == NULL)
        return -1;
    count = mixer_ctl_get_num_values(control);
    if (count != 2)
        return -1;
    if (mixer_ctl_set_value(control, 0, value) != 0)
        return -1;
    return mixer_ctl_set_value(control, 1, value);
}

static int force_safe_mute(struct mixer_ctl *mute, struct mixer_ctl *volume)
{
    int result = 0;
    if (set_stereo_control(mute, 1) != 0)
        result = -1;
    if (set_stereo_control(volume, SAFE_MUTED_VOLUME) != 0)
        result = -1;
    return result;
}

static void fill_tone(int16_t *samples, uint32_t frame_count)
{
    static const int16_t wave[16] = {
        0, 196, 362, 473, 512, 473, 362, 196,
        0, -196, -362, -473, -512, -473, -362, -196,
    };
    for (uint32_t i = 0; i < frame_count; ++i)
        samples[i] = wave[i % 16];
}

static int self_test(void)
{
    int16_t samples[32];
    fill_tone(samples, 32);
    for (size_t i = 0; i < 32; ++i)
        if (samples[i] > TONE_PEAK || samples[i] < -TONE_PEAK)
            return EXIT_FAILURE;
    zero_memory(samples, sizeof(samples));
    for (size_t i = 0; i < 32; ++i)
        if (samples[i] != 0)
            return EXIT_FAILURE;
    puts("self_test=passed");
    return EXIT_SUCCESS;
}

int main(int argc, char **argv)
{
    static const struct pcm_config config = {
        .channels = PLAYBACK_CHANNELS,
        .rate = PLAYBACK_RATE,
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
    struct mixer *mixer = NULL;
    struct mixer_ctl *mute = NULL;
    struct mixer_ctl *volume = NULL;
    struct pcm *pcm = NULL;
    int16_t *samples = NULL;
    int result = EXIT_FAILURE;

    if (argc == 2 && strcmp(argv[1], "--self-test") == 0)
        return self_test();
    if (argc != 1) {
        fputs("usage: onyx-kvm-audio-playback-probe [--self-test]\n", stderr);
        return EXIT_FAILURE;
    }

    action.sa_handler = bounded_timeout_handler;
    sigemptyset(&action.sa_mask);
    if (sigaction(SIGALRM, &action, NULL) != 0) {
        perror("sigaction");
        return EXIT_FAILURE;
    }
    alarm(HARD_TIMEOUT_SECONDS);

    samples = calloc(1, PLAYBACK_BYTES);
    if (samples == NULL || mlock(samples, PLAYBACK_BYTES) != 0) {
        perror("playback_buffer");
        goto cleanup;
    }
    fill_tone(samples, PLAYBACK_FRAMES);

    mixer = mixer_open(PLAYBACK_CARD);
    if (mixer == NULL) {
        fputs("mixer_open_failed\n", stderr);
        goto cleanup;
    }
    mute = mixer_get_ctl_by_name(mixer, "DAC Playback MUTE");
    volume = mixer_get_ctl_by_name(mixer, "DAC Playback Volume");
    if (force_safe_mute(mute, volume) != 0) {
        fputs("initial_safe_mute_failed\n", stderr);
        goto cleanup;
    }

    pcm = pcm_open(PLAYBACK_CARD, PLAYBACK_DEVICE, PCM_OUT, &config);
    if (pcm == NULL || !pcm_is_ready(pcm)) {
        fprintf(stderr, "pcm_not_ready=%s\n", pcm == NULL ? "allocation" : pcm_get_error(pcm));
        goto cleanup;
    }
    if (set_stereo_control(volume, TEST_VOLUME) != 0 ||
        set_stereo_control(mute, 0) != 0) {
        fputs("bounded_unmute_failed\n", stderr);
        goto cleanup;
    }
    if (pcm_writei(pcm, samples, PLAYBACK_FRAMES) != PLAYBACK_FRAMES) {
        fprintf(stderr, "pcm_write_failed=%s\n", pcm_get_error(pcm));
        goto cleanup;
    }
    if (ioctl(pcm_get_file_descriptor(pcm), SNDRV_PCM_IOCTL_DRAIN) != 0) {
        perror("pcm_drain");
        goto cleanup;
    }
    if (force_safe_mute(mute, volume) != 0) {
        fputs("final_safe_mute_failed\n", stderr);
        goto cleanup;
    }

    printf("playback_frames=%u\n", PLAYBACK_FRAMES);
    printf("playback_bytes=%u\n", PLAYBACK_BYTES);
    printf("rate_hz=%u\n", PLAYBACK_RATE);
    printf("channels=%u\n", PLAYBACK_CHANNELS);
    printf("format=s16_le\n");
    printf("tone_hz=%u\n", PLAYBACK_RATE / 16);
    printf("tone_peak=%u\n", TONE_PEAK);
    printf("dac_control=%u\n", TEST_VOLUME);
    result = EXIT_SUCCESS;

cleanup:
    if (mute != NULL && volume != NULL && force_safe_mute(mute, volume) != 0)
        fputs("cleanup_safe_mute_failed\n", stderr);
    if (pcm != NULL)
        pcm_close(pcm);
    if (mixer != NULL)
        mixer_close(mixer);
    if (samples != NULL) {
        zero_memory(samples, PLAYBACK_BYTES);
        (void)munlock(samples, PLAYBACK_BYTES);
        free(samples);
    }
    alarm(0);
    if (playback_timed_out) {
        fputs("playback_timeout\n", stderr);
        return 124;
    }
    return result;
}
