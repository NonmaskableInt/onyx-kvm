/* SPDX-License-Identifier: MIT */
/* One-shot, local-only SG2002/LT6911 YUV capture admission probe. */
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/socket.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>

#include <linux/audit.h>
#include <linux/filter.h>
#include <linux/i2c-dev.h>
#include <linux/seccomp.h>

#include <linux/cif_uapi.h>
#include <linux/cvi_comm_vi.h>
#include <linux/cvi_comm_video.h>
#include <linux/cvi_common.h>
#include <linux/cvi_type.h>
#include "cvi_buffer.h"
#include "cvi_sys.h"
#include "cvi_vb.h"
#include "cvi_vi.h"

#define WIDTH 1920U
#define HEIGHT 1080U
#define OUTPUT_LIMIT (WIDTH * HEIGHT * 2U)

/* cvi_vi.o also contains optional GDC/register-dump entry points. The probe
 * never calls them; fail-closed stubs keep those unrelated implementations
 * and their ISP dependency graph out of the executable. */
CVI_S32 CVI_GDC_GenLDCMesh(void)
{
	return CVI_FAILURE;
}

CVI_S32 dump_hw_register(void)
{
	return CVI_FAILURE;
}

static int check(const char *name, CVI_S32 rc)
{
	if (rc == CVI_SUCCESS)
		return 0;
	fprintf(stderr, "%s failed: %#x\n", name, (unsigned int)rc);
	return -1;
}

static int i2c_write_pair(int fd, uint8_t reg, uint8_t value)
{
	uint8_t pair[2] = {reg, value};

	return write(fd, pair, sizeof(pair)) == (ssize_t)sizeof(pair) ? 0 : -1;
}

static int i2c_read_reg(int fd, uint8_t reg, uint8_t *value)
{
	return write(fd, &reg, 1) == 1 && read(fd, value, 1) == 1 ? 0 : -1;
}

static int lt6911_read_unlocked(int fd, uint8_t bank, uint8_t reg,
				uint8_t *value)
{
	int rc;

	/* Match the stock SG2002 driver exactly: each logical register read is
	 * bracketed by its own access-enable and access-disable sequence. */
	rc = i2c_write_pair(fd, 0xff, 0x80) ||
	     i2c_write_pair(fd, 0xee, 0x01) ||
	     i2c_write_pair(fd, 0xff, bank) ||
	     i2c_read_reg(fd, reg, value);
	if (i2c_write_pair(fd, 0xff, 0x80) ||
	    i2c_write_pair(fd, 0xee, 0x00))
		rc = -1;
	return rc;
}

static int probe_lt6911_identity(unsigned int bus, unsigned int address,
				 const char *state)
{
	char path[32];
	int fd = -1, rc = -1;
	uint8_t id_hi = 0, id_lo = 0;

	snprintf(path, sizeof(path), "/dev/i2c-%u", bus);
	fd = open(path, O_RDWR | O_CLOEXEC);
	if (fd < 0 || ioctl(fd, I2C_SLAVE_FORCE, address) < 0) {
		printf("LT6911_%s_BUS_%u_ADDR_%02x_OPEN_SELECT_ERRNO=%d\n",
		       state, bus, address, errno);
		goto out;
	}
	if (lt6911_read_unlocked(fd, 0xa0, 0x00, &id_hi) ||
	    lt6911_read_unlocked(fd, 0xa0, 0x01, &id_lo)) {
		printf("LT6911_%s_BUS_%u_ADDR_%02x_READ_ERRNO=%d\n",
		       state, bus, address, errno);
		goto out;
	}
	printf("LT6911_%s_BUS_%u_ADDR_%02x_CHIP_ID=%02x%02x\n",
	       state, bus, address, id_hi, id_lo);
	rc = 0;
out:
	if (fd >= 0)
		close(fd);
	return rc;
}

static void report_reset_registers(const char *state)
{
	static const off_t page = 0x03001000;
	int fd;
	volatile uint32_t *regs;

	fd = open("/dev/mem", O_RDONLY | O_CLOEXEC | O_SYNC);
	if (fd < 0) {
		printf("RESET_REG_%s_OPEN_ERRNO=%d\n", state, errno);
		return;
	}
	regs = mmap(NULL, 0x21000, PROT_READ, MAP_SHARED, fd, page);
	if (regs == MAP_FAILED) {
		printf("RESET_REG_%s_MMAP_ERRNO=%d\n", state, errno);
		close(fd);
		return;
	}
	/* ADC1 mux is 0x030010f8. GPIO B registers are based at 0x03021000:
	 * output data +0x00, direction +0x04, sampled external port +0x50. */
	printf("RESET_REG_%s_MUX=%08x OUT=%08x DIR=%08x SAMPLE=%08x\n",
	       state, regs[0x00f8 / 4], regs[0x20000 / 4],
	       regs[0x20004 / 4], regs[0x20050 / 4]);
	munmap((void *)regs, 0x21000);
	close(fd);
}

static int install_boundary(void)
{
	#define DENY_SYSCALL(nr, error) \
		BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, (nr), 0, 1), \
		BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | (error))
	struct sock_filter filter[] = {
		BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, arch)),
		BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, AUDIT_ARCH_RISCV64, 1, 0),
		BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),
		BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, nr)),
		DENY_SYSCALL(__NR_execve, EPERM),
		DENY_SYSCALL(__NR_execveat, EPERM),
		BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, __NR_socket, 0, 4),
		BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, args[0])),
		BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, AF_INET, 1, 0),
		BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, AF_INET6, 0, 1),
		BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | EAFNOSUPPORT),
		BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
	};
	const struct sock_fprog program = {
		.len = (unsigned short)(sizeof(filter) / sizeof(filter[0])),
		.filter = filter,
	};

	if (prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) < 0)
		return -1;
	return prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &program);
	#undef DENY_SYSCALL
}

int main(int argc, char **argv)
{
	const char *output = argc == 2 ? argv[1] : "/run/cleankvm-frame.yuv422p";
	VB_CONFIG_S vb = {0};
	VI_DEV_ATTR_S dev = {0};
	VI_DEV_BIND_PIPE_S bind = {0};
	VI_PIPE_ATTR_S pipe = {0};
	VI_CHN_ATTR_S chn = {0};
	VIDEO_FRAME_INFO_S frame = {0};
	struct combo_dev_attr_s mipi = {0};
	CVI_U32 mipi_dev = 0;
	CVI_U32 sensor_reset_dev = 0;
	int mipi_fd = -1, out_fd = -1, rc = 1;
	int sys_up = 0, dev_up = 0, pipe_created = 0, pipe_up = 0, chn_up = 0;
	unsigned int i;

	if (argc > 2) {
		fprintf(stderr, "usage: %s [output]\n", argv[0]);
		return 2;
	}
	if (install_boundary() < 0) {
		perror("install syscall boundary");
		return 1;
	}
	vb.u32MaxPoolCnt = 1;
	vb.astCommPool[0].u32BlkSize = COMMON_GetPicBufferSize(
		WIDTH, HEIGHT, PIXEL_FORMAT_YUV_PLANAR_422, DATA_BITWIDTH_8,
		COMPRESS_MODE_NONE, DEFAULT_ALIGN);
	vb.astCommPool[0].u32BlkCnt = 2;
	vb.astCommPool[0].enRemapMode = VB_REMAP_MODE_CACHED;
	(void)CVI_SYS_Exit();
	(void)CVI_VB_Exit();
	if (check("CVI_VB_SetConfig", CVI_VB_SetConfig(&vb)) ||
	    check("CVI_VB_Init", CVI_VB_Init()) ||
	    check("CVI_SYS_Init", CVI_SYS_Init()))
		goto cleanup;
	sys_up = 1;

	mipi.input_mode = INPUT_MODE_MIPI;
	mipi.mac_clk = RX_MAC_CLK_600M;
	mipi.mclk.cam = 0;
	mipi.mclk.freq = CAMPLL_FREQ_NONE;
	mipi.mipi_attr.raw_data_type = YUV422_8BIT;
	mipi.mipi_attr.lane_id[0] = 2;
	mipi.mipi_attr.lane_id[1] = 4;
	mipi.mipi_attr.lane_id[2] = 3;
	mipi.mipi_attr.lane_id[3] = 1;
	mipi.mipi_attr.lane_id[4] = 0;
	for (i = 0; i < 5; ++i)
		mipi.mipi_attr.pn_swap[i] = 0;
	mipi.mipi_attr.wdr_mode = CVI_MIPI_WDR_MODE_NONE;
	mipi.mipi_attr.dphy.enable = 1;
	mipi.mipi_attr.dphy.hs_settle = 8;
	mipi.devno = mipi_dev;
	mipi.img_size.width = WIDTH;
	mipi.img_size.height = HEIGHT;

	mipi_fd = open("/dev/cvi-mipi-rx", O_RDWR | O_CLOEXEC);
	if (mipi_fd < 0) {
		perror("open /dev/cvi-mipi-rx");
		goto cleanup;
	}
	report_reset_registers("BOOT");
	(void)probe_lt6911_identity(4, 0x2b, "BOOT");
	(void)probe_lt6911_identity(4, 0x2c, "BOOT");
	/* The Mars driver reset operation asserts and deasserts both PHY resets and
	 * toggles the MAC reset. Its UAPI marks UNRESET_MIPI unsupported. */
	/* Match the stock Lite/PCIe utility's bounded HDMI reset pulse: active
	 * for 100 ms, then released for 100 ms. */
	if (ioctl(mipi_fd, CVI_MIPI_RESET_SENSOR, &sensor_reset_dev) < 0) {
		perror("LT6911 reset assert");
		goto cleanup;
	}
	usleep(100000);
	report_reset_registers("ASSERTED");
	(void)probe_lt6911_identity(4, 0x2b, "ASSERTED");
	(void)probe_lt6911_identity(4, 0x2c, "ASSERTED");
	if (ioctl(mipi_fd, CVI_MIPI_UNRESET_SENSOR, &sensor_reset_dev) < 0) {
		perror("LT6911 reset release");
		goto cleanup;
	}
	usleep(500000);
	report_reset_registers("RELEASED");
	(void)probe_lt6911_identity(3, 0x2b, "RELEASED");
	(void)probe_lt6911_identity(4, 0x2b, "RELEASED");
	(void)probe_lt6911_identity(4, 0x2c, "RELEASED");
	fflush(stdout);
	/* Configure only the bounded MIPI receiver using the stock NanoKVM Lite
	 * sensor_cfg.ini.LT wiring profile. Stop before the vendor VI capture path,
	 * whose failed-frame cleanup can block indefinitely. */
	if (ioctl(mipi_fd, CVI_MIPI_RESET_MIPI, &mipi_dev) < 0 ||
	    ioctl(mipi_fd, CVI_MIPI_SET_DEV_ATTR, &mipi) < 0) {
		perror("MIPI configuration");
		goto cleanup;
	}
	usleep(500000);
	return 0;

	dev.enIntfMode = VI_MODE_MIPI_YUV422;
	dev.enWorkMode = VI_WORK_MODE_1Multiplex;
	dev.enScanMode = VI_SCAN_PROGRESSIVE;
	for (i = 0; i < VI_MAX_ADCHN_NUM; ++i)
		dev.as32AdChnId[i] = -1;
	dev.enDataSeq = VI_DATA_SEQ_UYVY;
	dev.enInputDataType = VI_DATA_TYPE_YUV;
	dev.stSize.u32Width = WIDTH;
	dev.stSize.u32Height = HEIGHT;
	dev.stWDRAttr.enWDRMode = WDR_MODE_NONE;
	dev.enBayerFormat = BAYER_FORMAT_BG;
	dev.snrFps = 60;
	if (check("CVI_VI_SetDevAttr", CVI_VI_SetDevAttr(0, &dev)) ||
	    check("CVI_VI_EnableDev", CVI_VI_EnableDev(0)))
		goto cleanup;
	dev_up = 1;

	bind.u32Num = 1;
	bind.PipeId[0] = 0;
	if (check("CVI_VI_SetDevBindPipe", CVI_VI_SetDevBindPipe(0, &bind)))
		goto cleanup;

	pipe.enPipeBypassMode = VI_PIPE_BYPASS_NONE;
	pipe.bIspBypass = CVI_TRUE;
	pipe.u32MaxW = WIDTH;
	pipe.u32MaxH = HEIGHT;
	pipe.enPixFmt = PIXEL_FORMAT_YUV_PLANAR_422;
	pipe.enCompressMode = COMPRESS_MODE_NONE;
	pipe.enBitWidth = DATA_BITWIDTH_8;
	pipe.bNrEn = CVI_FALSE;
	pipe.bSharpenEn = CVI_FALSE;
	pipe.stFrameRate.s32SrcFrameRate = -1;
	pipe.stFrameRate.s32DstFrameRate = -1;
	pipe.bYuvBypassPath = CVI_TRUE;
	if (check("CVI_VI_CreatePipe", CVI_VI_CreatePipe(0, &pipe)))
		goto cleanup;
	pipe_created = 1;
	if (check("CVI_VI_StartPipe", CVI_VI_StartPipe(0)))
		goto cleanup;
	pipe_up = 1;

	chn.stSize.u32Width = WIDTH;
	chn.stSize.u32Height = HEIGHT;
	chn.enPixelFormat = PIXEL_FORMAT_YUV_PLANAR_422;
	chn.enDynamicRange = DYNAMIC_RANGE_SDR8;
	chn.enVideoFormat = VIDEO_FORMAT_LINEAR;
	chn.enCompressMode = COMPRESS_MODE_NONE;
	chn.u32Depth = 1;
	chn.stFrameRate.s32SrcFrameRate = -1;
	chn.stFrameRate.s32DstFrameRate = -1;
	chn.u32BindVbPool = VB_INVALID_POOLID;
	if (check("CVI_VI_SetChnAttr", CVI_VI_SetChnAttr(0, 0, &chn)) ||
	    check("CVI_VI_EnableChn", CVI_VI_EnableChn(0, 0)))
		goto cleanup;
	chn_up = 1;

	if (check("CVI_VI_GetChnFrame", CVI_VI_GetChnFrame(0, 0, &frame, 3000)))
		goto cleanup;
	if (frame.stVFrame.u32Width != WIDTH || frame.stVFrame.u32Height != HEIGHT ||
	    frame.stVFrame.enCompressMode != COMPRESS_MODE_NONE) {
		fprintf(stderr, "unexpected frame geometry or compression\n");
		goto release;
	}
	out_fd = open(output, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
	if (out_fd < 0) {
		perror("open output");
		goto release;
	}
	for (i = 0; i < 3; ++i) {
		void *copy;
		CVI_U32 length = frame.stVFrame.u32Length[i];
		ssize_t written;
		if (length == 0)
			continue;
		if (length > OUTPUT_LIMIT) {
			fprintf(stderr, "plane %u exceeds bound\n", i);
			goto release;
		}
		copy = malloc(length);
		if (!copy) {
			fprintf(stderr, "allocate plane %u failed\n", i);
			goto release;
		}
		if (CVI_VB_CopyBlock(frame.stVFrame.u64PhyAddr[0],
			(CVI_U32)(frame.stVFrame.u64PhyAddr[i] -
			 frame.stVFrame.u64PhyAddr[0]), copy, length) != CVI_SUCCESS) {
			fprintf(stderr, "copy plane %u failed\n", i);
			free(copy);
			goto release;
		}
		written = write(out_fd, copy, length);
		memset(copy, 0, length);
		free(copy);
		if (written != (ssize_t)length) {
			perror("write frame");
			goto release;
		}
	}
	if (fsync(out_fd) < 0) {
		perror("fsync frame");
		goto release;
	}
	printf("captured=%ux%u format=%d bytes<=%u output=%s\n", WIDTH, HEIGHT,
	       frame.stVFrame.enPixelFormat, OUTPUT_LIMIT, output);
	rc = 0;

release:
	(void)CVI_VI_ReleaseChnFrame(0, 0, &frame);
cleanup:
	if (out_fd >= 0)
		close(out_fd);
	if (chn_up)
		(void)CVI_VI_DisableChn(0, 0);
	if (pipe_up)
		(void)CVI_VI_StopPipe(0);
	if (pipe_created)
		(void)CVI_VI_DestroyPipe(0);
	if (dev_up)
		(void)CVI_VI_DisableDev(0);
	if (mipi_fd >= 0)
		close(mipi_fd);
	if (sys_up) {
		(void)CVI_SYS_Exit();
		(void)CVI_VB_Exit();
	}
	return rc;
}
