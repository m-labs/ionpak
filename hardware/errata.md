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
* R214 -> 4.7k
* LM339PT is in TSSOP package. Change for SOIC P/N
* GDT200 minimum firing voltage is too low
* the relay model should be changed to 9001-05-00. 9001-05-02 is an undocumented normally closed variant.
* D504 should follow the schematics in its datasheet
* R502 should be connected to VCC
* R502 -> 240 ohm
* R234 -> 7.5k
* Q104 -> DMN3404
* R114 -> 470 ohm
* R115 -> 4.7k
* R227 -> 470 ohm
* R228 -> 4.7k
* review values of R226 and R113
* use crystal type recommended by the MCU datasheet
* fix filament voltage protection threshold
* 22k FBI current resistor -> 10k?
* add connector for OLED display?
