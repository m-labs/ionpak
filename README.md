![ionpak](https://raw.githubusercontent.com/m-labs/ionpak/master/logo.svg)

A modern, low-cost universal controller for hot-cathode ionization vacuum gauges.

![Test setup](https://raw.githubusercontent.com/m-labs/ionpak/master/pictures/setup1_resized.jpg)

Why?
----

Many physics experiments require a good vacuum and therefore tools to diagnose vacuum-related problems. Ionization gauges provide a way to measure pressures in the medium to ultra-high vacuum regimes.

While the gauges themselves can be procured quite easily, the controller situation is more problematic. Existing commercial and DIY solutions have one or more of the following issues:

 * expensive
 * proprietary
 * poor performance at low (pA) collector currents
 * designed with obsolete components and overly bulky
 * lack of gauge voltage/current programmability
 * no/poor connectivity to a logging database
 * poor design and construction

The ionpak features:

 * a single-board low-cost design with modern components
 * open hardware and software
 * sub-pA electrometer performance
 * configurable voltages and emission current to adapt to various gauges
 * Ethernet connectivity

Where to find ionization vacuum gauges?
---------------------------------------

For medium and high vacuum, Chinese gauges can be ordered on Taobao under references ZJ-27 and ZJ-10. The Chinese for ionization gauge is 电离规. Those gauges have a simple construction and an extremely low cost, under USD 10 for glass tubulated models (requires flameworking) and a few times that price for flange-mounted ones. Those manufactured by [Chengdu Zhenghua](http://www.zhvacuum.com) appear to work satisfactorily, and glass gauges arrive sealed under vacuum (around 10⁻⁵ to 10⁻⁶ mbar) so they can be tested immediately. Ceramic sockets for the gauges can often be ordered from the same Taobao vendor.

For ultra-high vacuum, you can use G8130 from KJLC, which is pricy but of excellent quality. There is a lower-cost Bayard-Alpert Chinese gauge ZJ-12, of dubious quality for those pressure ranges, and with 2.5x less sensitivity than the KJLC part.

A low-cost source of quality UHV gauges remains to be found. You can try eBay, but the condition of the items is highly random.

How to connect the gauge?
-------------------------

The collector must be connected with a shielded (e.g. coaxial) cable. If the gauge has a metallic body (e.g. gauges mounted on CF flanges), connect the shield of the cable to the body, so that current from the high voltage electrodes will not leak into the collector electrode. This should not cause any ground loop in your system, as the gauge circuits inside the ionpak are galvanically isolated from its power input and from its Ethernet connector.

If a single cable is used for the filament and anode connections, the anode wire must be particularly well insulated from the filament wires, to avoid leakage currents that can be significant when the gauge is operated with a low emission (100µA or less) and influence the measurement result.

"Domino" terminal blocks from the local hardware store, with the plastic removed, can be used to connect the cables to the pins of CF-flanged gauges. Female circular contacts, such as Amphenol AT62-16-0122, are a nicer alternative but are slightly harder to find.

For baked systems, use PTFE insulated cables that can withstand 200°C. Operating the gauge during bake-out can remove the need to degas it (see the paper [Comments on the stability of B-A ionization gages](https://www.nist.gov/sites/default/files/documents/calibrations/jv13-2.pdf)).

It is a good idea to attach the cables to the vacuum system with a strap to avoid damaging the gauge by accidentally pulling on the cables, and to place a cover onto the pins to reduce the electrical shock hazard from the high voltage. Preferably those items should be made of metal so that they can be left on during bakeout.

Warning
-------

Ionization gauges use dangerous voltages and the ionpak is capable of delivering a lethal amount of power. Be careful and use at your own risk.

Building and loading the firmware
---------------------------------

```sh
cd firmware
openocd -f support/openocd.cfg
xargo build --release
arm-none-eabi-gdb -x support/load.gdb target/thumbv7em-none-eabihf/release/ionpak-firmware
```

License
-------

Copyright 2017 M-Labs Limited.

License TBD, probably will be CERN OHL and GPL.

Pictures in the ``pictures`` folder, except ``proto_rev1.jpg``, are copyright Michelle Poon and not free.
