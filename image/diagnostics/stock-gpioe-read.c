/* Offline-built, stock-only, read-only GPIOE9 snapshot. Never package in CleanKVM. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/mman.h>
#include <unistd.h>

struct register_read {
	const char *name;
	off_t address;
};

static const struct register_read reads[] = {
	{ "PWR_ON_MUX", 0x0300109c },
	{ "PWR_GPIO0_MUX", 0x030010a4 },
	{ "PWR_GPIO2_MUX", 0x030010ac },
	{ "GPIOE_OUTPUT", 0x05021000 },
	{ "GPIOE_DIRECTION", 0x05021004 },
	{ "GPIOE_INPUT", 0x05021050 },
};

int main(void)
{
	int fd = open("/dev/mem", O_RDONLY | O_CLOEXEC);
	size_t i;
	long page_size = sysconf(_SC_PAGESIZE);

	if (fd < 0) {
		perror("open /dev/mem read-only");
		return 1;
	}
	if (page_size != 4096) {
		fprintf(stderr, "unexpected page size: %ld\n", page_size);
		close(fd);
		return 1;
	}
	for (i = 0; i < sizeof(reads) / sizeof(reads[0]); i++) {
		off_t page = reads[i].address & ~((off_t)page_size - 1);
		size_t offset = reads[i].address - page;
		void *mapping = mmap(NULL, page_size, PROT_READ, MAP_SHARED, fd, page);
		uint32_t value;

		if (mapping == MAP_FAILED) {
			perror(reads[i].name);
			close(fd);
			return 1;
		}
		value = *(volatile uint32_t *)((char *)mapping + offset);
		printf("%s=0x%08" PRIx32 "\n", reads[i].name, value);
		if (munmap(mapping, page_size) != 0) {
			perror("munmap");
			close(fd);
			return 1;
		}
	}
	if (close(fd) != 0) {
		perror("close /dev/mem");
		return 1;
	}
	return 0;
}
