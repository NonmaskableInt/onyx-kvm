# Phase 3 MIPI signal measurement

Status: prepared; no physical waveform capture has been made.

## Purpose

Compare the LT6911-to-SG2002 MIPI D-PHY signals under stock and Onyx KVM boots
without changing bridge or receiver state. Static receiver controls, pad
electrical words, clocks, reset routing, and pin mux are already equal or
causally closed. The remaining repeatable difference is live PHY behavior,
especially logical D3.

## Source-backed connector map

The official LicheeRV Nano 70405 schematic labels the 24-pin camera connector
as FPC2. The stock product profile orders logical lanes as
`{CLK,D0,D1,D2,D3} = {RX2,RX4,RX3,RX1,RX0}`.

| Logical signal | SG2002 physical pair | FPC2 N pin | FPC2 P pin | Priority |
| --- | --- | ---: | ---: | --- |
| Clock | MIPI RX2 | 12 | 13 | Reference |
| D0 | MIPI RX4 | 6 | 7 | Control |
| D1 | MIPI RX3 | 9 | 10 | Control |
| D2 | MIPI RX1 | 15 | 16 | Control |
| D3 | MIPI RX0 | 18 | 19 | First data measurement |

FPC2 ground pins adjacent to the pairs are 8, 11, 14, 17, and 20. The pair
must be measured differentially; these grounds are references and are not a
substitute for one side of a differential measurement.

Authoritative board source:
<https://cn.dl.sipeed.com/fileList/LICHEE/LicheeRV_Nano/02_Schematic/LicheeRV_Nano-70405_Schematic.pdf>

The related official NanoKVM material identifies Lite as the LicheeRV Nano E
plus the HDMI-to-CSI board:
<https://wiki.sipeed.com/nanokvm>

## Pin-mux equivalence

Offline RISC-V disassembly of the extracted stock and Onyx KVM second-stage
loaders shows the same operation for all ten mux words at `0x0300116c` through
`0x03001190`: read the word, clear bits 0..2, set value 3, and write it back.
The sequences cover RX4N/P, RX3N/P, RX2N/P, RX1N/P, and RX0N/P. Their locations
are stock `0x80201c3e..0x80201caa` and Onyx KVM
`0x80200d9e..0x80200e0a` in the retained disassemblies. This closes MIPI pin
mux as a stock/Onyx KVM discriminator without another target mutation.

## Equipment and connection boundary

Use a high-impedance, low-capacitance active differential probe and a scope or
CSI analyzer with bandwidth and sample rate sufficient for this link. Attach
the probe with power removed and mechanically secure it before boot. Do not
attach a passive probe ground clip to either member of a MIPI pair, bridge
adjacent FPC contacts, or hot-probe the exposed connector.

If FPC2 contacts cannot be accessed without disturbing the cable, use a proper
24-pin interposer or identify paired test pads by power-off continuity. Do not
infer test-pad identity from position alone.

## Fixed comparison conditions

- Use the same NanoKVM, HDMI source, video mode, cable, power supply, probe,
  channel settings, and physical probe placement for both boots.
- Use 1920x1080 at the already tested source refresh rate.
- Let each image reach its steady receiver state before capture.
- Do not run an LT6911 write, reset pulse, EDID update, or receiver tuning sweep.
- Record the image identity and the simultaneous `/proc/mipi-rx` state.

## Capture order

1. Measure clock RX2 N/P at FPC2 pins 12/13 under stock, then Onyx KVM.
2. Measure D3/RX0 N/P at pins 18/19 under stock, then Onyx KVM.
3. If D3 differs, repeat once after a full power cycle before moving the probe.
4. Measure D0/RX4 at pins 6/7 as a control pair.
5. Measure D1/RX3 and D2/RX1 only if clock, D3, and D0 do not isolate the fault.

For each pair, retain an overview capture showing LP/HS transitions and a
short HS acquisition at identical timebase, bandwidth limit, termination, and
vertical scale. Record differential peak-to-peak amplitude, common-mode level
if the probe supports it, transition activity, and whether the pair enters HS.

## Interpretation gate

- If stock and Onyx KVM waveforms match at the connector while receiver status
  differs, resume SoC-side live-state analysis; do not alter the bridge.
- If D3 differs while clock and D0 match, localize the difference toward the
  LT6911 output state or the RX0 interconnect before changing receiver code.
- If all pairs differ, recheck the fixed source conditions and bridge state.
- Admit no register write from waveform shape alone. A follow-up mutation needs
  corresponding-source evidence and one bounded hardware hypothesis.

Video remains fail-closed until a complete frame and the existing cleanup and
security gates pass.
