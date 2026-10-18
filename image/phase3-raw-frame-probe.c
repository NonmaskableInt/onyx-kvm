/* SPDX-License-Identifier: MIT */
/* One-shot, local-only SG2002/LT6911 YUV capture admission probe. */
#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include <sys/prctl.h>
#include <sys/socket.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>

static int write_all(int fd, const void *buf, size_t count)
{
	const char *p = (const char *)buf;
	while (count > 0) {
		ssize_t written = write(fd, p, count);
		if (written <= 0) {
			if (written < 0 && (errno == EINTR || errno == EAGAIN))
				continue;
			return -1;
		}
		p += written;
		count -= (size_t)written;
	}
	return 0;
}

static volatile sig_atomic_t g_stop = 0;
static void handle_sig(int sig)
{
	(void)sig;
	g_stop = 1;
}

#include <linux/audit.h>
#include <linux/filter.h>
#include <linux/seccomp.h>

#include <linux/cif_uapi.h>
#include <linux/cvi_comm_vi.h>
#include <linux/cvi_comm_video.h>
#include <linux/cvi_common.h>
#include <linux/cvi_type.h>
#include <linux/vi_uapi.h>
#include <linux/vi_isp.h>
#include "cvi_buffer.h"
#include "cvi_sys.h"
#include "cvi_vb.h"
#include "cvi_vi.h"
#include "cvi_vpss.h"
#include <linux/cvi_comm_venc.h>
#include <linux/cvi_comm_rc.h>
#include <linux/cvi_vc_drv_ioctl.h>

#ifndef _VIDEO_FRAME_INFO_EX_S_DEFINED
typedef struct _VIDEO_FRAME_INFO_EX_S {
	VIDEO_FRAME_INFO_S *pstFrame;
	CVI_S32 s32MilliSec;
} VIDEO_FRAME_INFO_EX_S;
#define _VIDEO_FRAME_INFO_EX_S_DEFINED
#endif

extern CVI_S32 get_vi_fd(CVI_VOID);

#define WIDTH 1920U
#define HEIGHT 1080U
#define OUTPUT_LIMIT (WIDTH * HEIGHT * 2U)
#define H264_STREAM_LIMIT (1024U * 1024U)

static int acquire_output_frame(int vpss_up, VIDEO_FRAME_INFO_S *out_frame, unsigned int timeout_ms)
{
	VIDEO_FRAME_INFO_S vi_frame = {0};
	if (CVI_VI_GetChnFrame(0, 0, &vi_frame, timeout_ms) != CVI_SUCCESS)
		return -1;
	if (!vpss_up) {
		*out_frame = vi_frame;
		return 0;
	}
	if (CVI_VPSS_SendFrame(0, &vi_frame, 1000) != CVI_SUCCESS) {
		(void)CVI_VI_ReleaseChnFrame(0, 0, &vi_frame);
		return -1;
	}
	(void)CVI_VI_ReleaseChnFrame(0, 0, &vi_frame);
	if (CVI_VPSS_GetChnFrame(0, 0, out_frame, 1000) != CVI_SUCCESS)
		return -1;
	return 0;
}

static void release_output_frame(int vpss_up, VIDEO_FRAME_INFO_S *out_frame)
{
	if (vpss_up) {
		(void)CVI_VPSS_ReleaseChnFrame(0, 0, out_frame);
	} else {
		(void)CVI_VI_ReleaseChnFrame(0, 0, out_frame);
	}
}

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

static int snapshot_mipi(unsigned int elapsed_ms)
{
	char buffer[4096];
	char label[64];
	ssize_t count, written;
	int fd;

	written = snprintf(label, sizeof(label),
		"[CLEANKVM_MIPI_T%u_MS]\n", elapsed_ms);
	if (written < 0 || written >= (ssize_t)sizeof(label) ||
	    write(STDOUT_FILENO, label, (size_t)written) != written)
		return -1;
	fd = open("/proc/mipi-rx", O_RDONLY | O_CLOEXEC);
	if (fd < 0)
		return -1;
	while ((count = read(fd, buffer, sizeof(buffer))) > 0) {
		if (write(STDOUT_FILENO, buffer, (size_t)count) != count) {
			close(fd);
			return -1;
		}
	}
	close(fd);
	return count < 0 ? -1 : 0;
}

static int snapshot_proc(const char *path, const char *tag)
{
	char buffer[4096];
	char label[128];
	ssize_t count, written;
	int fd;

	written = snprintf(label, sizeof(label), "[CLEANKVM_%s]\n", tag);
	if (written < 0 || written >= (ssize_t)sizeof(label) ||
	    write(STDOUT_FILENO, label, (size_t)written) != written)
		return -1;
	fd = open(path, O_RDONLY | O_CLOEXEC);
	if (fd < 0)
		return -1;
	while ((count = read(fd, buffer, sizeof(buffer))) > 0) {
		if (write(STDOUT_FILENO, buffer, (size_t)count) != count) {
			close(fd);
			return -1;
		}
	}
	close(fd);
	return count < 0 ? -1 : 0;
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
	static const struct {
		int16_t lane_id[5];
		uint8_t pn_swap[5];
		CVI_U32 mipi_dev;
	} lane_profiles[] = {
		/* 0: Baseline 4-lane profile (clock=2, lanes 4,3,1,0) */
		{{2, 4, 3, 1, 0}, {0, 0, 0, 0, 0}, 0},
		/* 1..5: Previous permutation profiles */
		{{3, 0, 1, 2, 4}, {0, 0, 0, 0, 0}, 0},
		{{1, 4, 3, 2, 0}, {0, 0, 0, 0, 0}, 0},
		{{2, 0, 1, 3, 4}, {0, 0, 0, 0, 0}, 0},
		{{3, 1, 2, 4, 0}, {0, 0, 0, 0, 0}, 0},
		{{2, 0, 1, 3, 4}, {1, 1, 1, 1, 1}, 1},
		/* 6: 2-lane MIPI (clk=2, data0=1, data1=0) - zero PN swap */
		{{2, 1, 0, -1, -1}, {0, 0, 0, 0, 0}, 0},
		/* 7: 2-lane MIPI (clk=2, data0=0, data1=1) - zero PN swap */
		{{2, 0, 1, -1, -1}, {0, 0, 0, 0, 0}, 0},
		/* 8: 2-lane MIPI (clk=2, data0=1, data1=0) - PN swap both data lanes */
		{{2, 1, 0, -1, -1}, {0, 1, 1, 0, 0}, 0},
		/* 9: 2-lane MIPI (clk=2, data0=0, data1=1) - PN swap both data lanes */
		{{2, 0, 1, -1, -1}, {0, 1, 1, 0, 0}, 0},
		/* 10: 2-lane MIPI (clk=2, data0=1, data1=0) - PN swap data0 only */
		{{2, 1, 0, -1, -1}, {0, 1, 0, 0, 0}, 0},
		/* 11: 2-lane MIPI (clk=2, data0=1, data1=0) - PN swap data1 only */
		{{2, 1, 0, -1, -1}, {0, 0, 1, 0, 0}, 0},
		/* 12: 2-lane MIPI (clk=2, data0=0, data1=1) - PN swap data0 only */
		{{2, 0, 1, -1, -1}, {0, 1, 0, 0, 0}, 0},
		/* 13: 2-lane MIPI (clk=2, data0=0, data1=1) - PN swap data1 only */
		{{2, 0, 1, -1, -1}, {0, 0, 1, 0, 0}, 0},
		/* 14: 4-lane MIPI (clk=2, 4,3,1,0) - PN swap D3 (phys 0) */
		{{2, 4, 3, 1, 0}, {0, 0, 0, 0, 1}, 0},
		/* 15: 4-lane MIPI (clk=2, 4,3,1,0) - PN swap D0 (phys 4) */
		{{2, 4, 3, 1, 0}, {0, 1, 0, 0, 0}, 0},
		/* 16: 4-lane MIPI (clk=2, 4,3,1,0) - PN swap D1 (phys 3) */
		{{2, 4, 3, 1, 0}, {0, 0, 1, 0, 0}, 0},
		/* 17: 4-lane MIPI (clk=2, 4,3,1,0) - PN swap D2 (phys 1) */
		{{2, 4, 3, 1, 0}, {0, 0, 0, 1, 0}, 0},
		/* 18: 4-lane MIPI (clk=2, 4,3,1,0) - PN swap D0, D3 (phys 4, 0) */
		{{2, 4, 3, 1, 0}, {0, 1, 0, 0, 1}, 0},
		/* 19: 4-lane MIPI (clk=2, 4,3,1,0) - PN swap D1, D3 (phys 3, 0) */
		{{2, 4, 3, 1, 0}, {0, 0, 1, 0, 1}, 0},
		/* 20: 4-lane MIPI (clk=2, 4,3,1,0) - PN swap D0, D1, D3 (phys 4, 3, 0) */
		{{2, 4, 3, 1, 0}, {0, 1, 1, 0, 1}, 0},
		/* 21: 4-lane MIPI (clk=2, 4,3,1,0) - PN swap all 4 data lanes */
		{{2, 4, 3, 1, 0}, {0, 1, 1, 1, 1}, 0},
		/* 22: 4-lane MIPI (clk=2, 0,1,3,4) - zero PN swap */
		{{2, 0, 1, 3, 4}, {0, 0, 0, 0, 0}, 0},
		/* 23: 4-lane MIPI (clk=2, 0,1,3,4) - PN swap D0 (phys 0) */
		{{2, 0, 1, 3, 4}, {0, 1, 0, 0, 0}, 0},
		/* 24: 4-lane MIPI (clk=2, 0,1,3,4) - PN swap all data lanes */
		{{2, 0, 1, 3, 4}, {0, 1, 1, 1, 1}, 0},
	};
	const char *output = "/run/cleankvm-frame.yuv422p";
	VB_CONFIG_S vb = {0};
	VI_DEV_ATTR_S dev = {0};
	VI_DEV_BIND_PIPE_S bind = {0};
	VI_PIPE_ATTR_S pipe = {0};
	VI_CHN_ATTR_S chn = {0};
	VIDEO_FRAME_INFO_S frame = {0};
	struct combo_dev_attr_s mipi = {0};
	CVI_U32 mipi_dev = 0;
	int mipi_fd = -1, out_fd = -1, rc = 1;
	int sys_up = 0, dev_up = 0, pipe_created = 0, pipe_up = 0, chn_up = 0, vpss_up = 0;
	int h264_mode = 0;
	int venc_fd = -1, venc_created = 0, venc_started = 0;
	uint8_t *h264_buf = NULL;
	unsigned int i, lane_profile = 0;
	CVI_U32 target_height = HEIGHT;
	CVI_U32 out_width = WIDTH;
	CVI_U32 out_height = HEIGHT;
	int vpss_needed = 0;
	uint8_t target_hs_settle = 16;
	const char *override_pn = NULL;
	const char *override_lanes = NULL;

	int stream_mode = 0;
	struct sigaction sa;

	if (argc > 8) {
		fprintf(stderr, "usage: %s [lane-profile-0..%zu] [height] [hs_settle] [pn_swap] [lane_id] [mode] [output]\n", argv[0],
			sizeof(lane_profiles) / sizeof(lane_profiles[0]) - 1);
		return 2;
	}
	if (argc >= 2) {
		char *end = NULL;
		unsigned long selected = strtoul(argv[1], &end, 10);

		if (!argv[1][0] || !end || *end || selected >=
		    sizeof(lane_profiles) / sizeof(lane_profiles[0])) {
			fprintf(stderr, "invalid lane profile: %s\n", argv[1]);
			return 2;
		}
		lane_profile = (unsigned int)selected;
	}
	if (argc >= 3) {
		char *end = NULL;
		unsigned long selected = strtoul(argv[2], &end, 10);

		if (!argv[2][0] || !end || *end || (selected != 1080 && selected != 720)) {
			fprintf(stderr, "invalid height: %s (must be 1080 or 720)\n", argv[2]);
			return 2;
		}
		target_height = (CVI_U32)selected;
	}
	if (target_height == 720) {
		out_width = 1280;
		out_height = 720;
		vpss_needed = 1;
	} else {
		out_width = 1920;
		out_height = 1080;
		vpss_needed = 0;
	}
	if (argc >= 4) {
		char *end = NULL;
		unsigned long selected = strtoul(argv[3], &end, 10);

		if (!argv[3][0] || !end || *end || selected < 1 || selected > 255) {
			fprintf(stderr, "invalid hs_settle: %s\n", argv[3]);
			return 2;
		}
		target_hs_settle = (uint8_t)selected;
	}
	if (argc >= 5 && argv[4][0]) {
		override_pn = argv[4];
	}
	if (argc >= 6 && argv[5][0]) {
		override_lanes = argv[5];
	}
	if (argc >= 7 && argv[6][0]) {
		if (strcmp(argv[6], "stream") == 0) {
			stream_mode = 1;
		} else if (strcmp(argv[6], "stream-h264") == 0) {
			stream_mode = 1;
			h264_mode = 1;
		} else if (strcmp(argv[6], "oneshot-h264") == 0) {
			stream_mode = 0;
			h264_mode = 1;
		} else if (strcmp(argv[6], "oneshot") == 0) {
			stream_mode = 0;
		} else {
			output = argv[6];
			stream_mode = 0;
		}
	}
	if (h264_mode) {
		vpss_needed = 1;
	}
	if (argc >= 8 && argv[7][0]) {
		output = argv[7];
	}
	memset(&sa, 0, sizeof(sa));
	sa.sa_handler = handle_sig;
	sigaction(SIGINT, &sa, NULL);
	sigaction(SIGTERM, &sa, NULL);
	signal(SIGPIPE, SIG_IGN);
	if (install_boundary() < 0) {
		perror("install syscall boundary");
		return 1;
	}
	if (vpss_needed) {
		vb.u32MaxPoolCnt = 2;
		vb.astCommPool[0].u32BlkSize = COMMON_GetPicBufferSize(
			WIDTH, HEIGHT, PIXEL_FORMAT_UYVY, DATA_BITWIDTH_8,
			COMPRESS_MODE_NONE, DEFAULT_ALIGN);
		vb.astCommPool[0].u32BlkCnt = 4;
		vb.astCommPool[0].enRemapMode = VB_REMAP_MODE_CACHED;
		vb.astCommPool[1].u32BlkSize = COMMON_GetPicBufferSize(
			out_width, out_height,
			h264_mode ? PIXEL_FORMAT_YUV_PLANAR_420 : PIXEL_FORMAT_UYVY,
			DATA_BITWIDTH_8,
			COMPRESS_MODE_NONE, DEFAULT_ALIGN);
		vb.astCommPool[1].u32BlkCnt = 4;
		vb.astCommPool[1].enRemapMode = VB_REMAP_MODE_CACHED;
	} else {
		vb.u32MaxPoolCnt = 1;
		vb.astCommPool[0].u32BlkSize = COMMON_GetPicBufferSize(
			WIDTH, HEIGHT, PIXEL_FORMAT_UYVY, DATA_BITWIDTH_8,
			COMPRESS_MODE_NONE, DEFAULT_ALIGN);
		vb.astCommPool[0].u32BlkCnt = 4;
		vb.astCommPool[0].enRemapMode = VB_REMAP_MODE_CACHED;
	}
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
	mipi_dev = lane_profiles[lane_profile].mipi_dev;
	for (i = 0; i < 5; ++i) {
		mipi.mipi_attr.lane_id[i] = lane_profiles[lane_profile].lane_id[i];
		mipi.mipi_attr.pn_swap[i] = lane_profiles[lane_profile].pn_swap[i];
	}
	if (override_pn) {
		unsigned int p0 = 0, p1 = 0, p2 = 0, p3 = 0, p4 = 0;
		if (sscanf(override_pn, "%u,%u,%u,%u,%u", &p0, &p1, &p2, &p3, &p4) == 5) {
			mipi.mipi_attr.pn_swap[0] = (uint8_t)p0;
			mipi.mipi_attr.pn_swap[1] = (uint8_t)p1;
			mipi.mipi_attr.pn_swap[2] = (uint8_t)p2;
			mipi.mipi_attr.pn_swap[3] = (uint8_t)p3;
			mipi.mipi_attr.pn_swap[4] = (uint8_t)p4;
		} else {
			fprintf(stderr, "invalid pn_swap override: %s\n", override_pn);
			goto cleanup;
		}
	}
	if (override_lanes) {
		int l0 = 0, l1 = 0, l2 = 0, l3 = 0, l4 = 0;
		if (sscanf(override_lanes, "%d,%d,%d,%d,%d", &l0, &l1, &l2, &l3, &l4) == 5) {
			mipi.mipi_attr.lane_id[0] = (int16_t)l0;
			mipi.mipi_attr.lane_id[1] = (int16_t)l1;
			mipi.mipi_attr.lane_id[2] = (int16_t)l2;
			mipi.mipi_attr.lane_id[3] = (int16_t)l3;
			mipi.mipi_attr.lane_id[4] = (int16_t)l4;
		} else {
			fprintf(stderr, "invalid lane_id override: %s\n", override_lanes);
			goto cleanup;
		}
	}
	mipi.mipi_attr.wdr_mode = CVI_MIPI_WDR_MODE_NONE;
	mipi.mipi_attr.dphy.enable = 1;
	mipi.mipi_attr.dphy.hs_settle = target_hs_settle;
	mipi.devno = mipi_dev;
	mipi.img_size.width = WIDTH;
	mipi.img_size.height = HEIGHT;
	if (!stream_mode) {
		printf("CLEANKVM_LANE_PROFILE=%u DEV=%u TARGET_HEIGHT=%u HS_SETTLE=%u PN=%u,%u,%u,%u,%u LANES=%d,%d,%d,%d,%d\n",
			lane_profile, mipi_dev, target_height, target_hs_settle,
			mipi.mipi_attr.pn_swap[0], mipi.mipi_attr.pn_swap[1],
			mipi.mipi_attr.pn_swap[2], mipi.mipi_attr.pn_swap[3],
			mipi.mipi_attr.pn_swap[4],
			mipi.mipi_attr.lane_id[0], mipi.mipi_attr.lane_id[1],
			mipi.mipi_attr.lane_id[2], mipi.mipi_attr.lane_id[3],
			mipi.mipi_attr.lane_id[4]);
	} else {
		fprintf(stderr, "CLEANKVM_LANE_PROFILE=%u DEV=%u TARGET_HEIGHT=%u HS_SETTLE=%u PN=%u,%u,%u,%u,%u LANES=%d,%d,%d,%d,%d\n",
			lane_profile, mipi_dev, target_height, target_hs_settle,
			mipi.mipi_attr.pn_swap[0], mipi.mipi_attr.pn_swap[1],
			mipi.mipi_attr.pn_swap[2], mipi.mipi_attr.pn_swap[3],
			mipi.mipi_attr.pn_swap[4],
			mipi.mipi_attr.lane_id[0], mipi.mipi_attr.lane_id[1],
			mipi.mipi_attr.lane_id[2], mipi.mipi_attr.lane_id[3],
			mipi.mipi_attr.lane_id[4]);
	}

	/* Step 1: Configure VI Dev 0 before MIPI RX so that clk_csi_mac0
	 * is enabled by CVI_VI_EnableDev before high-speed CSI packets arrive. */
	dev.enIntfMode = VI_MODE_MIPI_YUV422;
	dev.enWorkMode = VI_WORK_MODE_1Multiplex;
	dev.enScanMode = VI_SCAN_PROGRESSIVE;
	for (i = 0; i < VI_MAX_ADCHN_NUM; ++i)
		dev.as32AdChnId[i] = -1;
	dev.enDataSeq = VI_DATA_SEQ_UYVY;
	dev.enInputDataType = VI_DATA_TYPE_YUV;
	dev.stSynCfg.enVsync = VI_VSYNC_PULSE;
	dev.stSynCfg.enVsyncNeg = VI_VSYNC_NEG_LOW;
	dev.stSynCfg.enHsync = VI_HSYNC_VALID_SINGNAL;
	dev.stSynCfg.enHsyncNeg = VI_HSYNC_NEG_HIGH;
	dev.stSynCfg.enVsyncValid = VI_VSYNC_VALID_SIGNAL;
	dev.stSynCfg.enVsyncValidNeg = VI_VSYNC_VALID_NEG_HIGH;
	dev.stSynCfg.stTimingBlank.u32HsyncAct = WIDTH;
	dev.stSynCfg.stTimingBlank.u32VsyncVact = HEIGHT;
	dev.stSize.u32Width = WIDTH;
	dev.stSize.u32Height = HEIGHT;
	dev.stWDRAttr.enWDRMode = WDR_MODE_NONE;
	dev.stWDRAttr.u32CacheLine = HEIGHT;
	dev.enBayerFormat = BAYER_FORMAT_BG;
	dev.snrFps = 60;
	if (check("CVI_VI_SetDevAttr", CVI_VI_SetDevAttr(0, &dev)) ||
	    check("CVI_VI_EnableDev", CVI_VI_EnableDev(0)))
		goto cleanup;
	dev_up = 1;

	/* Step 1b: Configure sensor geometry on the VI kernel driver so the CSI bridge
	 * registers (CH0_SIZE) receive width=1920, height=1080 rather than 0x0.
	 * Without active_w/active_h configured, csibdg_width and csibdg_height
	 * default to 0, causing the CSI bridge to never assert FRAME_DONE_FE0. */
	{
		struct vi_ext_control ext = {0};
		struct cvi_isp_snr_info snr = {0};
		int vi_fd;

		snr.raw_num = 0;
		snr.color_mode = 0;
		snr.pixel_rate = 148500;
		snr.snr_fmt.frm_num = 1;
		snr.snr_fmt.img_size[0].width = WIDTH;
		snr.snr_fmt.img_size[0].height = HEIGHT;
		snr.snr_fmt.img_size[0].active_w = WIDTH;
		snr.snr_fmt.img_size[0].active_h = HEIGHT;
		snr.snr_fmt.img_size[0].max_width = WIDTH;
		snr.snr_fmt.img_size[0].max_height = HEIGHT;
		snr.snr_fmt.img_size[0].start_x = 0;
		snr.snr_fmt.img_size[0].start_y = 0;

		ext.id = VI_IOCTL_SET_SNR_INFO;
		ext.ptr = &snr;

		vi_fd = get_vi_fd();
		if (vi_fd < 0)
			vi_fd = open("/dev/cvi-vi", O_RDWR | O_CLOEXEC);
		if (vi_fd < 0) {
			perror("get_vi_fd / open /dev/cvi-vi");
			goto cleanup;
		}
		if (ioctl(vi_fd, VI_IOC_S_CTRL, &ext) < 0) {
			perror("ioctl VI_IOCTL_SET_SNR_INFO");
			goto cleanup;
		}
	}

	mipi_fd = open("/dev/cvi-mipi-rx", O_RDWR | O_CLOEXEC);
	if (mipi_fd < 0) {
		perror("open /dev/cvi-mipi-rx");
		goto cleanup;
	}
	/* Step 2: Reset only the SoC MIPI receiver and configure device attributes.
	 * Do NOT pulse sensor reset or sensor clock: the LT6911 bridge is already
	 * running, clocked by its onboard crystal, and locked to HDMI input. */
	if (ioctl(mipi_fd, CVI_MIPI_RESET_MIPI, &mipi_dev) < 0 ||
	    ioctl(mipi_fd, CVI_MIPI_SET_DEV_ATTR, &mipi) < 0) {
		perror("MIPI configuration");
		goto cleanup;
	}

	/* Step 3: Bind and start the single-frame VI pipe. */
	bind.u32Num = 1;
	bind.PipeId[0] = 0;
	if (check("CVI_VI_SetDevBindPipe", CVI_VI_SetDevBindPipe(0, &bind)))
		goto cleanup;

	pipe.enPipeBypassMode = VI_PIPE_BYPASS_NONE;
	pipe.bIspBypass = CVI_TRUE;
	pipe.u32MaxW = WIDTH;
	pipe.u32MaxH = HEIGHT;
	pipe.enPixFmt = PIXEL_FORMAT_UYVY;
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
	chn.enPixelFormat = PIXEL_FORMAT_UYVY;
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

	if (vpss_needed) {
		VPSS_GRP_ATTR_S grp_attr = {0};
		VPSS_CHN_ATTR_S chn_attr = {0};

		grp_attr.u32MaxW = WIDTH;
		grp_attr.u32MaxH = HEIGHT;
		grp_attr.enPixelFormat = PIXEL_FORMAT_UYVY;
		grp_attr.stFrameRate.s32SrcFrameRate = -1;
		grp_attr.stFrameRate.s32DstFrameRate = -1;
		grp_attr.u8VpssDev = 0;

		chn_attr.u32Width = out_width;
		chn_attr.u32Height = out_height;
		chn_attr.enVideoFormat = VIDEO_FORMAT_LINEAR;
		chn_attr.enPixelFormat = h264_mode ? PIXEL_FORMAT_YUV_PLANAR_420 : PIXEL_FORMAT_UYVY;
		chn_attr.stFrameRate.s32SrcFrameRate = -1;
		chn_attr.stFrameRate.s32DstFrameRate = -1;
		chn_attr.u32Depth = 1;
		chn_attr.bMirror = CVI_FALSE;
		chn_attr.bFlip = CVI_FALSE;
		chn_attr.stAspectRatio.enMode = ASPECT_RATIO_NONE;

		if (check("CVI_VPSS_CreateGrp", CVI_VPSS_CreateGrp(0, &grp_attr)) ||
		    check("CVI_VPSS_SetChnAttr", CVI_VPSS_SetChnAttr(0, 0, &chn_attr)) ||
		    check("CVI_VPSS_EnableChn", CVI_VPSS_EnableChn(0, 0)) ||
		    check("CVI_VPSS_StartGrp", CVI_VPSS_StartGrp(0)))
			goto cleanup;
		vpss_up = 1;
	}

	if (h264_mode) {
		VENC_CHN_ATTR_S venc_attr = {0};
		VENC_RECV_PIC_PARAM_S recv_param = {0};

		venc_fd = open("/dev/cvi_vc_enc0", O_RDWR | O_CLOEXEC);
		if (venc_fd < 0) {
			perror("open /dev/cvi_vc_enc0");
			goto cleanup;
		}

		venc_attr.stVencAttr.enType = PT_H264;
		venc_attr.stVencAttr.u32MaxPicWidth = out_width;
		venc_attr.stVencAttr.u32MaxPicHeight = out_height;
		venc_attr.stVencAttr.u32PicWidth = out_width;
		venc_attr.stVencAttr.u32PicHeight = out_height;
		venc_attr.stVencAttr.u32BufSize = 0;
		venc_attr.stVencAttr.u32Profile = H264E_PROFILE_BASELINE;
		venc_attr.stVencAttr.bByFrame = CVI_TRUE;
		venc_attr.stVencAttr.stAttrH264e.bRcnRefShareBuf = CVI_FALSE;
		venc_attr.stVencAttr.stAttrH264e.bSingleLumaBuf = CVI_FALSE;

		venc_attr.stRcAttr.enRcMode = VENC_RC_MODE_H264FIXQP;
		venc_attr.stRcAttr.stH264FixQp.u32Gop = 30;
		venc_attr.stRcAttr.stH264FixQp.u32SrcFrameRate = 30;
		venc_attr.stRcAttr.stH264FixQp.fr32DstFrameRate = 30;
		venc_attr.stRcAttr.stH264FixQp.bVariFpsEn = CVI_FALSE;
		venc_attr.stRcAttr.stH264FixQp.u32IQp = 26;
		venc_attr.stRcAttr.stH264FixQp.u32PQp = 28;

		venc_attr.stGopAttr.enGopMode = VENC_GOPMODE_NORMALP;
		venc_attr.stGopAttr.stNormalP.s32IPQpDelta = 2;

		if (ioctl(venc_fd, CVI_VC_VENC_CREATE_CHN, &venc_attr) < 0) {
			perror("ioctl CVI_VC_VENC_CREATE_CHN");
			goto cleanup;
		}
		venc_created = 1;

		recv_param.s32RecvPicNum = -1;
		if (ioctl(venc_fd, CVI_VC_VENC_START_RECV_FRAME, &recv_param) < 0) {
			perror("ioctl CVI_VC_VENC_START_RECV_FRAME");
			goto cleanup;
		}
		venc_started = 1;

		h264_buf = malloc(H264_STREAM_LIMIT);
		if (!h264_buf) {
			fprintf(stderr, "allocate h264 stream buffer failed\n");
			goto cleanup;
		}
	}

	if (stream_mode) {
		uint8_t *buf_a = NULL;
		uint8_t *buf_b = NULL;
		uint8_t *delta_buf = NULL;
		uint8_t *curr_buf = NULL;
		uint8_t *prev_buf = NULL;
		int first_delta_frame = 1;
		uint8_t ready_byte = 0;

		if (!h264_mode) {
			buf_a = malloc(OUTPUT_LIMIT);
			buf_b = malloc(OUTPUT_LIMIT);
			delta_buf = malloc(8 + 810 * 5122);
			if (!buf_a || !buf_b || !delta_buf) {
				fprintf(stderr, "allocate stream buffers failed\n");
				if (buf_a) free(buf_a);
				if (buf_b) free(buf_b);
				if (delta_buf) free(delta_buf);
				goto cleanup;
			}
			curr_buf = buf_a;
			prev_buf = buf_b;
		}

		if (write_all(STDOUT_FILENO, &ready_byte, 1) != 0) {
			if (buf_a) free(buf_a);
			if (buf_b) free(buf_b);
			if (delta_buf) free(delta_buf);
			goto cleanup;
		}

		int consecutive_timeouts = 0;
		while (!g_stop) {
			uint8_t cmd = 0;
			ssize_t n = read(STDIN_FILENO, &cmd, 1);
			if (n <= 0) {
				break;
			}
			if (cmd == 'Q') {
				break;
			}
			if (h264_mode || cmd == 3) {
				if (acquire_output_frame(vpss_up, &frame, 200) != 0) {
					consecutive_timeouts++;
					uint8_t err_header[5] = {1, 0, 0, 0, 0};
					if (write_all(STDOUT_FILENO, err_header, 5) != 0)
						break;
					if (consecutive_timeouts >= 8) {
						/* ~2.5 to 3.5s of persistent hardware timeout / signal loss.
						 * Break out to cleanup so hardware channels close cleanly. */
						break;
					}
					usleep(250000);
					continue;
				}
				consecutive_timeouts = 0;
				VIDEO_FRAME_INFO_EX_S frame_ex = {
					.pstFrame = &frame,
					.s32MilliSec = 1000,
				};
				int send_err = ioctl(venc_fd, CVI_VC_VENC_SEND_FRAME, &frame_ex);
				release_output_frame(vpss_up, &frame);
				if (send_err < 0) {
					uint8_t err_header[5] = {1, 0, 0, 0, 0};
					if (write_all(STDOUT_FILENO, err_header, 5) != 0)
						break;
					usleep(250000);
					continue;
				}
				ssize_t bytes_read = read(venc_fd, h264_buf, H264_STREAM_LIMIT);
				if (bytes_read <= 0) {
					uint8_t err_header[5] = {1, 0, 0, 0, 0};
					if (write_all(STDOUT_FILENO, err_header, 5) != 0)
						break;
					usleep(250000);
					continue;
				}
				uint8_t ok_header[5];
				ok_header[0] = 0;
				ok_header[1] = (uint8_t)(((uint32_t)bytes_read >> 24) & 0xff);
				ok_header[2] = (uint8_t)(((uint32_t)bytes_read >> 16) & 0xff);
				ok_header[3] = (uint8_t)(((uint32_t)bytes_read >> 8) & 0xff);
				ok_header[4] = (uint8_t)((uint32_t)bytes_read & 0xff);
				if (write_all(STDOUT_FILENO, ok_header, 5) != 0 ||
				    write_all(STDOUT_FILENO, h264_buf, (size_t)bytes_read) != 0) {
					break;
				}
				continue;
			}
			if (acquire_output_frame(vpss_up, &frame, 200) != 0) {
				consecutive_timeouts++;
				uint8_t err_header[5] = {1, 0, 0, 0, 0};
				first_delta_frame = 1;
				if (write_all(STDOUT_FILENO, err_header, 5) != 0)
					break;
				if (consecutive_timeouts >= 8) {
					break;
				}
				usleep(250000);
				continue;
			}
			consecutive_timeouts = 0;
			if (frame.stVFrame.u32Width != out_width || frame.stVFrame.u32Height != out_height) {
				release_output_frame(vpss_up, &frame);
				uint8_t err_header[5] = {1, 0, 0, 0, 0};
				first_delta_frame = 1;
				if (write_all(STDOUT_FILENO, err_header, 5) != 0)
					break;
				continue;
			}
			CVI_U32 total_bytes = 0;
			for (i = 0; i < 3; ++i) {
				total_bytes += frame.stVFrame.u32Length[i];
			}
			CVI_U32 curr_offset = 0;
			int copy_err = 0;
			for (i = 0; i < 3; ++i) {
				CVI_U32 length = frame.stVFrame.u32Length[i];
				if (length == 0)
					continue;
				if (curr_offset + length > OUTPUT_LIMIT ||
				    CVI_VB_CopyBlock(frame.stVFrame.u64PhyAddr[0],
					(CVI_U32)(frame.stVFrame.u64PhyAddr[i] -
					 frame.stVFrame.u64PhyAddr[0]),
					curr_buf + curr_offset, length) != CVI_SUCCESS) {
					copy_err = 1;
					break;
				}
				curr_offset += length;
			}
			release_output_frame(vpss_up, &frame);
			if (copy_err || curr_offset != total_bytes) {
				uint8_t err_header[5] = {1, 0, 0, 0, 0};
				first_delta_frame = 1;
				if (write_all(STDOUT_FILENO, err_header, 5) != 0)
					break;
				continue;
			}

			if (cmd != 2) {
				uint8_t ok_header[5];
				ok_header[0] = 0;
				ok_header[1] = (uint8_t)((total_bytes >> 24) & 0xff);
				ok_header[2] = (uint8_t)((total_bytes >> 16) & 0xff);
				ok_header[3] = (uint8_t)((total_bytes >> 8) & 0xff);
				ok_header[4] = (uint8_t)(total_bytes & 0xff);
				if (write_all(STDOUT_FILENO, ok_header, 5) != 0 ||
				    write_all(STDOUT_FILENO, curr_buf, total_bytes) != 0) {
					break;
				}
				first_delta_frame = 1;
				continue;
			}

			if (first_delta_frame) {
				uint8_t ok_header[5];
				ok_header[0] = 0;
				ok_header[1] = (uint8_t)((total_bytes >> 24) & 0xff);
				ok_header[2] = (uint8_t)((total_bytes >> 16) & 0xff);
				ok_header[3] = (uint8_t)((total_bytes >> 8) & 0xff);
				ok_header[4] = (uint8_t)(total_bytes & 0xff);
				if (write_all(STDOUT_FILENO, ok_header, 5) != 0 ||
				    write_all(STDOUT_FILENO, curr_buf, total_bytes) != 0) {
					break;
				}
				uint8_t *tmp = curr_buf;
				curr_buf = prev_buf;
				prev_buf = tmp;
				first_delta_frame = 0;
				continue;
			}

			uint16_t dirty_count = 0;
			uint8_t *delta_payload = delta_buf + 8;
			const uint16_t tile_cols = (uint16_t)(out_width / 64);
			const uint16_t tile_rows = (uint16_t)(out_height / 40);
			const uint16_t total_tiles = tile_cols * tile_rows;
			const uint16_t keyframe_threshold = total_tiles / 2;
			const uint32_t stride = out_width * 2;

			for (uint16_t t = 0; t < total_tiles; ++t) {
				uint16_t col = t % tile_cols;
				uint16_t row = t / tile_cols;
				int dirty = 0;
				for (uint32_t r = 0; r < 40; ++r) {
					const uint64_t *c = (const uint64_t *)(curr_buf + (row * 40 + r) * stride + col * 128);
					const uint64_t *p = (const uint64_t *)(prev_buf + (row * 40 + r) * stride + col * 128);
					for (int w = 0; w < 16; ++w) {
						if (c[w] != p[w]) {
							dirty = 1;
							break;
						}
					}
					if (dirty)
						break;
				}
				if (dirty) {
					if (dirty_count >= keyframe_threshold) {
						dirty_count = keyframe_threshold + 1;
						break;
					}
					delta_payload[0] = (uint8_t)((t >> 8) & 0xff);
					delta_payload[1] = (uint8_t)(t & 0xff);
					for (uint32_t r = 0; r < 40; ++r) {
						memcpy(delta_payload + 2 + r * 128,
						       curr_buf + (row * 40 + r) * stride + col * 128,
						       128);
					}
					delta_payload += (2 + 5120);
					dirty_count++;
				}
			}

			if (dirty_count > keyframe_threshold) {
				uint8_t ok_header[5];
				ok_header[0] = 0;
				ok_header[1] = (uint8_t)((total_bytes >> 24) & 0xff);
				ok_header[2] = (uint8_t)((total_bytes >> 16) & 0xff);
				ok_header[3] = (uint8_t)((total_bytes >> 8) & 0xff);
				ok_header[4] = (uint8_t)(total_bytes & 0xff);
				if (write_all(STDOUT_FILENO, ok_header, 5) != 0 ||
				    write_all(STDOUT_FILENO, curr_buf, total_bytes) != 0) {
					break;
				}
			} else {
				delta_buf[0] = (uint8_t)((dirty_count >> 8) & 0xff);
				delta_buf[1] = (uint8_t)(dirty_count & 0xff);
				delta_buf[2] = 0;
				delta_buf[3] = 64;
				delta_buf[4] = 0;
				delta_buf[5] = 40;
				delta_buf[6] = 0;
				delta_buf[7] = 0;
				uint32_t delta_len = 8 + (uint32_t)dirty_count * 5122;
				uint8_t ok_header[5];
				ok_header[0] = 0;
				ok_header[1] = (uint8_t)((delta_len >> 24) & 0xff);
				ok_header[2] = (uint8_t)((delta_len >> 16) & 0xff);
				ok_header[3] = (uint8_t)((delta_len >> 8) & 0xff);
				ok_header[4] = (uint8_t)(delta_len & 0xff);
				if (write_all(STDOUT_FILENO, ok_header, 5) != 0 ||
				    write_all(STDOUT_FILENO, delta_buf, delta_len) != 0) {
					break;
				}
			}
			uint8_t *tmp = curr_buf;
			curr_buf = prev_buf;
			prev_buf = tmp;
		}

		if (buf_a) {
			memset(buf_a, 0, OUTPUT_LIMIT);
			free(buf_a);
		}
		if (buf_b) {
			memset(buf_b, 0, OUTPUT_LIMIT);
			free(buf_b);
		}
		if (delta_buf) {
			memset(delta_buf, 0, 8 + 810 * 5122);
			free(delta_buf);
		}
		rc = 0;
		goto cleanup;
	}

	if (h264_mode) {
		if (acquire_output_frame(vpss_up, &frame, 5000) != 0) {
			fprintf(stderr, "oneshot h264 frame lock failed\n");
			goto cleanup;
		}
		VIDEO_FRAME_INFO_EX_S frame_ex = {
			.pstFrame = &frame,
			.s32MilliSec = 2000,
		};
		if (ioctl(venc_fd, CVI_VC_VENC_SEND_FRAME, &frame_ex) < 0) {
			perror("ioctl CVI_VC_VENC_SEND_FRAME");
			release_output_frame(vpss_up, &frame);
			goto cleanup;
		}
		release_output_frame(vpss_up, &frame);
		ssize_t bytes_read = read(venc_fd, h264_buf, H264_STREAM_LIMIT);
		if (bytes_read <= 0) {
			perror("read venc_fd");
			goto cleanup;
		}
		out_fd = open(output, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
		if (out_fd < 0) {
			perror("open output");
			goto cleanup;
		}
		if (write_all(out_fd, h264_buf, (size_t)bytes_read) != 0) {
			perror("write output");
			goto cleanup;
		}
		if (fsync(out_fd) < 0) {
			perror("fsync output");
			goto cleanup;
		}
		printf("captured=%ux%u format=h264 bytes=%zd output=%s\n", out_width, out_height, bytes_read, output);
		rc = 0;
		goto cleanup;
	}

	/* Observe receiver counters with active VI clocking and pipeline. */
	(void)snapshot_mipi(0);
	usleep(100000);
	(void)snapshot_mipi(100);
	(void)snapshot_proc("/proc/cvitek/vi", "VI_DEV");
	(void)snapshot_proc("/proc/cvitek/vi_dbg", "VI_DBG_PRE");

	if (acquire_output_frame(vpss_up, &frame, 5000) != 0) {
		(void)snapshot_proc("/proc/cvitek/vi_dbg", "VI_DBG_POST_ERR");
		(void)snapshot_mipi(5000);
		goto cleanup;
	}
	(void)snapshot_proc("/proc/cvitek/vi_dbg", "VI_DBG_POST_OK");
	(void)snapshot_mipi(500);
	if (frame.stVFrame.u32Width != out_width || frame.stVFrame.u32Height != out_height ||
	    frame.stVFrame.enCompressMode != COMPRESS_MODE_NONE) {
		fprintf(stderr, "unexpected frame geometry or compression\n");
		goto release;
	}
	out_fd = open(output, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
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
	printf("captured=%ux%u format=%d bytes<=%u output=%s\n", out_width, out_height,
	       frame.stVFrame.enPixelFormat, OUTPUT_LIMIT, output);
	rc = 0;

release:
	release_output_frame(vpss_up, &frame);
cleanup:
	if (h264_buf) {
		memset(h264_buf, 0, H264_STREAM_LIMIT);
		free(h264_buf);
	}
	if (venc_started) {
		(void)ioctl(venc_fd, CVI_VC_VENC_STOP_RECV_FRAME);
	}
	if (venc_created) {
		(void)ioctl(venc_fd, CVI_VC_VENC_DESTROY_CHN);
	}
	if (venc_fd >= 0)
		close(venc_fd);
	if (out_fd >= 0)
		close(out_fd);
	if (vpss_up) {
		(void)CVI_VPSS_StopGrp(0);
		(void)CVI_VPSS_DisableChn(0, 0);
		(void)CVI_VPSS_DestroyGrp(0);
	}
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
