ionpak
======

A modern, low-cost universal controller for ionization vacuum gauges.

![Prototype picture](https://raw.githubusercontent.com/m-labs/ionpak/master/proto_rev1_small.jpg)

Building and loading the firmware
---------------------------------

```sh
cd firmware
openocd -f support/openocd.cfg
xargo build --release
arm-none-eabi-gdb -x support/load.gdb target/thumbv7em-none-eabihf/release/ionpak-firmware
```

Flyback transformer construction
--------------------------------

TR300: Use EPCOS coilformer B66208X1010T1. Wind 5 turns on the primary and spread them across the length of the coilformer - it is particularly important that the air gap between the cores is covered by windings. Wind 70 turns on the secondary in multiple layers. As with all flyback transformers, the polarity of the windings is critical. Assemble with EPCOS cores B66317G500X127 and B66317GX127 (one half gapped core, one half ungapped core), and corresponding clips.

TR350: Use EPCOS coilformer B66206W1110T1 and cores B66311G250X127 and B66311GX127. Both the primary and the secondary have 5 turns and must be wound together, interleaving the windings. The same remarks as for TR300 apply.

Errata
------

PCB rev 1:

* R307 needs more clearance from D400
* Pins 1 and 12 of U502 need pull-downs
* Pin 1 of U501 needs pull-up
* D203 reversed polarity
* R236 and R234 are swapped
* Q301 needs to be NPN, change to BC817
* increase R307 -> 3.3Kohm and increase R300 -> 33Kohm
* C201: oscillates at 0 and 1nF, stable at 100nF
* add clamp diodes to GND on op-amp outputs to ADC when op-amp has negative supply
* R214 -> 4.7k
* LM339PT is in TSSOP package. Change for SOIC P/N
* GDT200 minimum firing voltage is too low
* the relay model should be changed to 9001-05-00. 9001-05-02 is an undocumented normally closed variant.
* D504 should follow the schematics in its datasheet
* R502 should be connected to VCC
