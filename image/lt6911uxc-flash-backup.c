/*
 * Bounded read-only LT6911UXC SPI survey for CleanKVM.
 *
 * The read transaction is derived from Sipeed NanoKVM's GPL-3.0-or-later
 * nanokvm_update_edid.c at bd070b2f5ce27d22618e8a1c8d2bdca543429882.
 * No erase/program opcode is present. The tool reads the same 800,000-byte
 * address range covered by Sipeed's published lt6911_read_fw() routine twice,
 * requires identical results, and writes one fixed root-only output.
 */
#define _DEFAULT_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/i2c-dev.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <unistd.h>

#define I2C_PATH "/dev/i2c-4"
#define OUTPUT_PATH "/run/cleankvm-lt6911-flash-800000.bin"
#define I2C_ADDRESS 0x2b
#define SYS 0x80
#define PAGE 16u
#define FLASH_BYTES 800000u

static int bus = -1;
static uint8_t bank = 0xff;

static int write_exact(const uint8_t *p, size_t n) {
    ssize_t rc = write(bus, p, n);
    if (rc == (ssize_t)n) return 0;
    if (rc >= 0) errno = EIO;
    return -1;
}

static int select_bank(uint8_t value) {
    uint8_t p[2] = {0xff, value};
    if (bank == value) return 0;
    if (write_exact(p, sizeof(p))) return -1;
    bank = value;
    return 0;
}

static int put(uint8_t reg, uint8_t value) {
    uint8_t p[2] = {reg, value};
    return select_bank(SYS) || write_exact(p, sizeof(p));
}

static int write_reg(uint8_t reg, uint8_t value) {
    uint8_t p[2] = {reg, value};
    return write_exact(p, sizeof(p));
}

static int read_page(uint32_t address, uint8_t *out) {
    uint8_t reg = 0x5f;
    if (select_bank(0x90) || write_reg(0x02,0xdf)) return -1;
    usleep(1000);
    if (write_reg(0x02,0xff) || put(0x5a,0x84)) return -1;
    usleep(1000);
    if (put(0x5a,0x80) || put(0x5e,0x6f) || put(0x5a,0xa0)) return -1;
    usleep(1000);
    if (put(0x5a,0x80) ||
        put(0x5b,(uint8_t)(address >> 16)) ||
        put(0x5c,(uint8_t)(address >> 8)) ||
        put(0x5d,(uint8_t)address) || put(0x5a,0x90)) return -1;
    usleep(5000);
    if (put(0x5a,0x80) || put(0x58,0x01) ||
        select_bank(SYS) || write_exact(&reg, 1)) return -1;
    return read(bus, out, PAGE) == PAGE ? 0 : -1;
}

static int survey(uint8_t *out) {
    uint32_t address;
    bank = 0xff;
    if (put(0xff,0x80) || put(0xee,0x01) || put(0x5a,0x80) ||
        put(0x5e,0xc0) || put(0x58,0x00) || put(0x59,0x51) ||
        put(0x5a,0x90))
        return -1;
    usleep(1000);
    if (put(0x5a,0x80)) return -1;
    for (address = 0; address < FLASH_BYTES; address += PAGE)
        if (read_page(address, out + address)) return -1;
    return 0;
}

static int resume_mcu(void) {
    bank = 0xff;
    return put(0xee, 0x00);
}

static int write_output(const uint8_t *data) {
    int out = open(OUTPUT_PATH, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0400);
    size_t done = 0;
    if (out < 0) return -1;
    while (done < FLASH_BYTES) {
        ssize_t rc = write(out, data + done, FLASH_BYTES - done);
        if (rc <= 0) { close(out); unlink(OUTPUT_PATH); return -1; }
        done += (size_t)rc;
    }
    if (fsync(out) || close(out)) { unlink(OUTPUT_PATH); return -1; }
    return 0;
}

int main(int argc, char **argv) {
    static uint8_t first[FLASH_BYTES], second[FLASH_BYTES];
    unsigned long non_ff = 0, non_zero = 0;
    size_t i;
    int result = 1;
    if (argc != 2 || strcmp(argv[1], "--dump-stock-range") != 0) {
        fprintf(stderr, "usage: %s --dump-stock-range\n", argv[0]);
        return 2;
    }
    if (geteuid() != 0 || access(OUTPUT_PATH, F_OK) == 0) {
        fputs("requires root and absent fixed output path\n", stderr);
        return 1;
    }
    bus = open(I2C_PATH, O_RDWR | O_CLOEXEC);
    if (bus < 0 || ioctl(bus, I2C_SLAVE, I2C_ADDRESS) < 0) {
        perror("open fixed LT6911UXC endpoint");
        goto out;
    }
    if (survey(first) || resume_mcu()) { perror("first read or MCU resume"); goto out; }
    usleep(100000);
    if (survey(second) || resume_mcu()) { perror("second read or MCU resume"); goto out; }
    if (memcmp(first, second, sizeof(first))) {
        fputs("two complete reads differ; output suppressed\n", stderr);
        goto out;
    }
    for (i = 0; i < sizeof(first); ++i) {
        if (first[i] != 0xff) ++non_ff;
        if (first[i] != 0x00) ++non_zero;
    }
    if (write_output(first)) { perror("write fixed backup"); goto out; }
    printf("verified_bytes=%u non_ff=%lu non_zero=%lu output=%s\n",
           FLASH_BYTES, non_ff, non_zero, OUTPUT_PATH);
    result = 0;
out:
    if (bus >= 0) {
        if (resume_mcu()) { perror("final MCU resume"); result = 1; }
        close(bus);
    }
    memset(first, 0, sizeof(first));
    memset(second, 0, sizeof(second));
    return result;
}
