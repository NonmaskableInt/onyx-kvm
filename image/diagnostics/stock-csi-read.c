/* Offline-built, stock-only, read-only CSI register snapshot. Never package. */
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

/*
 * Physical bases come from the pinned SG200x cif node. Offsets are named by
 * the pinned CIF register headers and are read by the independently built
 * driver diagnostics. Every mapping below is PROT_READ.
 */
static const struct register_read reads[] = {
	{ "PAD0_P", 0x03001c30 },
	{ "PAD0_N", 0x03001c34 },
	{ "PAD1_P", 0x03001c38 },
	{ "PAD1_N", 0x03001c3c },
	{ "PAD2_P", 0x03001c40 },
	{ "PAD2_N", 0x03001c44 },
	{ "PAD3_P", 0x03001c48 },
	{ "PAD3_N", 0x03001c4c },
	{ "PAD4_P", 0x03001c50 },
	{ "PAD4_N", 0x03001c54 },
	{ "PAD5_P", 0x03001c58 },
	{ "PAD5_N", 0x03001c5c },
	{ "CSI_CTRL_00", 0x0a0c2400 },
	{ "CSI_CTRL_04", 0x0a0c2404 },
	{ "CSI_CTRL_30", 0x0a0c2430 },
	{ "CSI_CTRL_34", 0x0a0c2434 },
	{ "CSI_CTRL_40", 0x0a0c2440 },
	{ "CSI_CTRL_48", 0x0a0c2448 },
	{ "CSI_CTRL_4C", 0x0a0c244c },
	{ "CSI_CTRL_50", 0x0a0c2450 },
	{ "CSI_CTRL_54", 0x0a0c2454 },
	{ "CSI_CTRL_58", 0x0a0c2458 },
	{ "CSI_CTRL_5C", 0x0a0c245c },
	{ "CSI_CTRL_60", 0x0a0c2460 },
	{ "CSI_CTRL_64", 0x0a0c2464 },
	{ "CSI_CTRL_70", 0x0a0c2470 },
	{ "CSI_CTRL_74", 0x0a0c2474 },
	{ "WRAP_TOP_80", 0x0a0d0080 },
	{ "WRAP_TOP_84", 0x0a0d0084 },
	{ "WRAP_TOP_88", 0x0a0d0088 },
	{ "PHY4L_00", 0x0a0d0300 },
	{ "PHY4L_04", 0x0a0d0304 },
	{ "PHY4L_08", 0x0a0d0308 },
	{ "PHY4L_0C", 0x0a0d030c },
	{ "PHY4L_10", 0x0a0d0310 },
	{ "PHY4L_20", 0x0a0d0320 },
	{ "PHY4L_24", 0x0a0d0324 },
	{ "PHY4L_28", 0x0a0d0328 },
	{ "PHY4L_30", 0x0a0d0330 },
	{ "PHY4L_34", 0x0a0d0334 },
	{ "PHY4L_38", 0x0a0d0338 },
	{ "PHY4L_3C", 0x0a0d033c },
	{ "PHY4L_40", 0x0a0d0340 },
	{ "PHY4L_44", 0x0a0d0344 },
	{ "PHY4L_48", 0x0a0d0348 },
	{ "PHY4L_4C", 0x0a0d034c },
	{ "PHY4L_50", 0x0a0d0350 },
	{ "PHY4L_54", 0x0a0d0354 },
	{ "PHY4L_58", 0x0a0d0358 },
	{ "PHY4L_5C", 0x0a0d035c },
	{ "PHY4L_60", 0x0a0d0360 },
	{ "PHY4L_64", 0x0a0d0364 },
	{ "PHY4L_DBG_90", 0x0a0d0390 },
	{ "PHY4L_DBG_94", 0x0a0d0394 },
	{ "PHY4L_STATUS_98", 0x0a0d0398 },
	{ "PHY4L_STATUS_9C", 0x0a0d039c },
	{ "PHY4L_STATUS_A4", 0x0a0d03a4 },
	{ "PHY4L_STATUS_A8", 0x0a0d03a8 },
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
