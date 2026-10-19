/*
 * Bounded LT6911UXC E21 EDID programmer for Onyx KVM.
 *
 * Derived from Sipeed NanoKVM nanokvm_update_edid.c at commit
 * bd070b2f5ce27d22618e8a1c8d2bdca543429882. GPL-3.0-or-later.
 * This version is intentionally limited to /dev/i2c-4, address 0x2b, the
 * embedded E21 payload, and the LT6911UXC protocol. It has no file, shell,
 * GPIO, product-selection, or arbitrary-register interface.
 */
#define _DEFAULT_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/i2c-dev.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <unistd.h>

#define I2C_PATH "/dev/i2c-4"
#define I2C_ADDRESS 0x2b
#define BANK_REG 0xff
#define SYS 0x80
#define STATUS 0x81
#define EDID_SIZE 256
#define PAGE_SIZE 32

static const uint8_t e21_edid[EDID_SIZE] = {
  0x00,0xff,0xff,0xff,0xff,0xff,0xff,0x00,0x4e,0x04,0x01,0x33,0x15,0x00,0x00,0x00,
  0x1e,0x23,0x01,0x03,0x80,0x30,0x1b,0x78,0x2a,0x69,0x25,0xa3,0x5b,0x50,0xa3,0x27,
  0x11,0x50,0x54,0xa5,0x4b,0x00,0xd1,0xc0,0x81,0x80,0x01,0x01,0x01,0x01,0x01,0x01,
  0x01,0x01,0x01,0x01,0x01,0x01,0x02,0x3a,0x80,0x18,0x71,0x38,0x2d,0x40,0x58,0x2c,
  0x45,0x00,0xdc,0x0c,0x11,0x00,0x00,0x1e,0x00,0x00,0x00,0xff,0x00,0x4e,0x61,0x6e,
  0x6f,0x4b,0x56,0x4d,0x0a,0x20,0x20,0x20,0x20,0x20,0x00,0x00,0x00,0xfc,0x00,0x4e,
  0x61,0x6e,0x6f,0x4b,0x56,0x4d,0x0a,0x20,0x20,0x20,0x20,0x20,0x00,0x00,0x00,0xfd,
  0x00,0x38,0x4c,0x1e,0x53,0x11,0x00,0x0a,0x20,0x20,0x20,0x20,0x20,0x20,0x01,0xf5,
  0x02,0x03,0x11,0x30,0x46,0x04,0x1f,0x14,0x13,0x01,0x10,0x65,0x03,0x0c,0x00,0x10,
  0x00,0x02,0x3a,0x80,0x18,0x71,0x38,0x2d,0x40,0x58,0x2c,0x45,0x00,0xdc,0x0c,0x11,
  0x00,0x00,0x1e,0x01,0x1d,0x00,0x72,0x51,0xd0,0x1e,0x20,0x6e,0x28,0x55,0x00,0xdc,
  0x0c,0x11,0x00,0x00,0x1e,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
  0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
  0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
  0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,
  0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0xda
};

static int fd = -1;
static uint8_t bank = 0xff;

static int write_all(const uint8_t *p, size_t n) {
    ssize_t got = write(fd, p, n);
    if (got == (ssize_t)n) return 0;
    if (got >= 0) errno = EIO;
    return -1;
}

static int select_bank(uint8_t value) {
    uint8_t b[2] = { BANK_REG, value };
    if (bank == value) return 0;
    if (write_all(b, sizeof(b))) return -1;
    bank = value;
    return 0;
}

static int put(uint8_t b, uint8_t reg, uint8_t value) {
    uint8_t p[2] = { reg, value };
    return select_bank(b) || write_all(p, sizeof(p));
}

static int put_block(uint8_t b, uint8_t reg, const uint8_t *data, size_t n) {
    uint8_t p[1 + PAGE_SIZE];
    if (n != PAGE_SIZE || select_bank(b)) return -1;
    p[0] = reg;
    memcpy(p + 1, data, n);
    return write_all(p, n + 1);
}

static int get_block(uint8_t b, uint8_t reg, uint8_t *data, size_t n) {
    if (select_bank(b) || write_all(&reg, 1)) return -1;
    return read(fd, data, n) == (ssize_t)n ? 0 : -1;
}

#define PUT(b,r,v) do { if (put((b),(r),(v))) goto io_fail; } while (0)

static int valid_edid(const uint8_t *p) {
    static const uint8_t header[8] = {0,255,255,255,255,255,255,0};
    unsigned block, i, sum;
    if (memcmp(p, header, sizeof(header)) || p[126] != 1) return 0;
    for (block = 0; block != 2; ++block) {
        for (sum = 0, i = 0; i != 128; ++i) sum += p[block * 128 + i];
        if ((sum & 0xff) != 0) return 0;
    }
    return 1;
}

static int read_edid(uint8_t *out) {
    unsigned i;
    PUT(SYS,0xff,0x80); PUT(SYS,0xee,0x01);
    PUT(SYS,0x5a,0x84); PUT(SYS,0x5a,0x80);
    for (i = 0; i != EDID_SIZE / PAGE_SIZE; ++i) {
        PUT(SYS,0x5e,0x5f); PUT(SYS,0x5a,0xa0); PUT(SYS,0x5a,0x80);
        PUT(SYS,0x5b,0x01); PUT(SYS,0x5c,0x80); PUT(SYS,0x5d,i * PAGE_SIZE);
        PUT(SYS,0x5a,0x90); PUT(SYS,0x5a,0x80); PUT(SYS,0x58,0x21);
        if (get_block(SYS, 0x5f, out + i * PAGE_SIZE, PAGE_SIZE)) goto io_fail;
    }
    return 0;
io_fail:
    return -1;
}

static int program_edid(const uint8_t *payload) {
    static const uint8_t zero_page[PAGE_SIZE] = {0};
    uint8_t status = 0;
    unsigned i;
    PUT(SYS,0xff,0x80); PUT(SYS,0xee,0x01); PUT(SYS,0x5e,0xdf);
    PUT(SYS,0x58,0x00); PUT(SYS,0x59,0x51); PUT(SYS,0x5a,0x10);
    PUT(SYS,0x5a,0x00); PUT(SYS,0x58,0x21); PUT(SYS,0xff,0x80);
    PUT(SYS,0xee,0x01); PUT(SYS,0x5a,0x80); PUT(SYS,0x5a,0x84);
    PUT(SYS,0x5a,0x80); PUT(SYS,0x5b,0x01); PUT(SYS,0x5c,0x80);
    PUT(SYS,0x5d,0x00); PUT(SYS,0x5a,0x81); PUT(SYS,0x5a,0x80);
    usleep(500000);
    if (get_block(STATUS, 0x08, &status, 1) || status != 0xee) {
        fprintf(stderr, "LT6911UXC erase-status gate failed: 0x%02x\n", status);
        return -1;
    }
    PUT(STATUS,0x08,0xae); PUT(STATUS,0x08,0xee);
    PUT(SYS,0xff,0x80); PUT(SYS,0xee,0x01); PUT(SYS,0x5a,0x84);
    PUT(SYS,0x5a,0x80); PUT(SYS,0x5a,0x84); PUT(SYS,0x5a,0x80);
    for (i = 0; i != 9; ++i) {
        PUT(SYS,0x5e,0xdf); PUT(SYS,0x5a,0x20); PUT(SYS,0x5a,0x00); PUT(SYS,0x58,0x21);
        if (put_block(SYS, 0x59, i == 8 ? zero_page : payload + i * PAGE_SIZE, PAGE_SIZE)) goto io_fail;
        PUT(SYS,0x5b,0x01); PUT(SYS,0x5c,i == 8 ? 0x81 : 0x80); PUT(SYS,0x5d,i == 8 ? 0 : i * PAGE_SIZE);
        PUT(SYS,0x5e,0xc0); PUT(SYS,0x5a,0x90); PUT(SYS,0x5a,0x80);
        PUT(SYS,0x5a,i == 8 ? 0x88 : 0x84); PUT(SYS,0x5a,0x80);
    }
    if (get_block(STATUS, 0x08, &status, 1) || status != 0xee) return -1;
    PUT(STATUS,0x08,0xae); PUT(STATUS,0x08,0xee);
    return 0;
io_fail:
    return -1;
}

static int prepare_named_edid(uint8_t *out, const char *name) {
    size_t len = strlen(name);
    size_t i;
    unsigned sum;
    uint8_t desc[13];

    if (len == 0 || len > 13) {
        fprintf(stderr, "error: EDID name length must be between 1 and 13 ASCII characters\n");
        return -1;
    }
    for (i = 0; i < len; ++i) {
        if ((unsigned char)name[i] < 32 || (unsigned char)name[i] > 126) {
            fprintf(stderr, "error: EDID name must contain only printable ASCII characters\n");
            return -1;
        }
    }
    memcpy(out, e21_edid, EDID_SIZE);

    memcpy(desc, name, len);
    if (len < 13) {
        desc[len] = 0x0a;
        for (i = len + 1; i < 13; ++i) {
            desc[i] = 0x20;
        }
    }

    /* Descriptor 2 (offset 72): tag 0xFF (Display Serial Number) -> text at offset 77..89 */
    memcpy(out + 77, desc, 13);
    /* Descriptor 3 (offset 90): tag 0xFC (Display Product Name) -> text at offset 95..107 */
    memcpy(out + 95, desc, 13);

    /* Recalculate Block 0 checksum at byte 127 */
    sum = 0;
    for (i = 0; i < 127; ++i) {
        sum += out[i];
    }
    out[127] = (uint8_t)((256 - (sum & 0xff)) & 0xff);

    if (!valid_edid(out)) {
        fprintf(stderr, "error: constructed EDID failed structural validation\n");
        return -1;
    }
    return 0;
}

int main(int argc, char **argv) {
    uint8_t before[EDID_SIZE], after[EDID_SIZE];
    uint8_t custom_edid[EDID_SIZE];
    const uint8_t *target_payload = NULL;
    int do_read_only = 0;
    int result = 1;

    if (argc == 2 && strcmp(argv[1], "--program-e21") == 0) {
        target_payload = e21_edid;
    } else if (argc == 3 && strcmp(argv[1], "--program-name") == 0) {
        if (prepare_named_edid(custom_edid, argv[2])) {
            return 1;
        }
        target_payload = custom_edid;
    } else if (argc == 2 && strcmp(argv[1], "--read") == 0) {
        do_read_only = 1;
    } else {
        fprintf(stderr, "usage: %s [--program-e21 | --program-name <NAME> | --read]\n", argv[0]);
        return 2;
    }

    if (target_payload != NULL && !valid_edid(target_payload)) {
        fputs("target EDID validation failed\n", stderr);
        return 1;
    }

    fd = open(I2C_PATH, O_RDWR | O_CLOEXEC);
    if (fd < 0 || ioctl(fd, I2C_SLAVE, I2C_ADDRESS) < 0) {
        perror("opening fixed LT6911UXC endpoint");
        goto out;
    }
    if (read_edid(before)) {
        perror("pre-write EDID read");
        goto resume;
    }

    if (do_read_only) {
        char name[14];
        int i;
        memset(name, 0, sizeof(name));
        for (i = 0; i < 13; ++i) {
            uint8_t c = before[95 + i];
            if (c == 0x0a || c < 32 || c > 126) break;
            name[i] = (char)c;
        }
        printf("current_edid_valid=%s\n", valid_edid(before) ? "true" : "false");
        printf("current_product_name=%s\n", name);
        result = 0;
        goto resume;
    }

    printf("pre-write-edid=%s\n", valid_edid(before) ? "valid" : "invalid");
    if (!memcmp(before, target_payload, EDID_SIZE)) {
        puts("Target EDID already installed; no write performed");
        result = 0;
        goto resume;
    }
    if (program_edid(target_payload)) {
        perror("bounded EDID program sequence");
        goto resume;
    }
    sleep(1);
    if (read_edid(after) || memcmp(after, target_payload, EDID_SIZE)) {
        fputs("post-write byte-for-byte verification failed\n", stderr);
        goto resume;
    }
    puts("Target EDID programmed and verified; power cycle HDMI source to re-enumerate");
    result = 0;

resume:
    bank = 0xff;
    if (put(SYS, 0xee, 0x00)) {
        perror("LT6911UXC MCU resume");
        result = 1;
    }
out:
    if (fd >= 0) close(fd);
    return result;
}
