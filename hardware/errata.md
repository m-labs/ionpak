Hardware errata
===============

Rev 1
-----

* R307 needs more clearance from D400
* Pins 1 and 12 of U502 need pull-downs
* Pin 1 of U501 needs pull-up
* D203 reversed polarity
* R236 and R234 are swapped
* Q301 needs to be NPN, change to BC817
* increase R307 -> 3.3Kohm and increase R500 -> 33Kohm
* C201: oscillates at 0 and 1nF, stable at 100nF
* add clamp diodes to GND on op-amp outputs to ADC when op-amp has negative supply
* R214 -> 7.5k
* LM339PT is in TSSOP package. Change for SOIC P/N
* GDT200 minimum firing voltage is too low
* the relay model should be changed to 9001-05-00. 9001-05-02 is an undocumented normally closed variant.
* D504 should follow the schematics in its datasheet
* R502 should be connected to VCC
* R502 -> 240 ohm
* C500 -> 1nF
* R234 -> 7.5k
* Q104 -> DMN3404
* R114 -> 470 ohm
* R115 -> 4.7k
* R227 -> 470 ohm
* R228 -> 4.7k
* review values of R226 and R113
* C100 -> 47pF NP0 AVX, R107 -> 100K, C101 -> 0.1uF
* use crystal type recommended by the MCU datasheet
* add connector for OLED display?
* enlarge holes to fit M3 screws comfortably
* add 15M resistor between A and FIL-
* power U200 from a small negative voltage instead of GND
* review choice of filament flyback output diode
* invert LED position so that the Ethernet LED is closest to the connector
* invert polarity of LEDs (Ethernet LED polarity cannot be programmed)
* change D351 model to MBR1645
* add snubber on D351
* add heatsink (Seifert-KK633) to D351
* move D351 further away from transformer for heatsink clearance
* add plated-through holes and pads for mounting rods of BNC connector
* simplify FBI circuit, 2 ranges only (typ. 8mA and 45mA), remove diodes and neglect MOSFET leakage
* to support different case designs, replace the holes on the connectors side with slots. One of the case designs requires holes whose center is at 11.5mm from the board edge.
