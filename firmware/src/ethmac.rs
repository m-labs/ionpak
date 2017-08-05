use cortex_m;
use tm4c129x;

use core::slice;
use smoltcp::Error;
use smoltcp::wire::EthernetAddress;
use smoltcp::phy::{DeviceLimits, Device};

const EPHY_BMCR: u8 =           0x00; // Ethernet PHY Basic Mode Control
#[allow(dead_code)]
const EPHY_BMSR: u8 =           0x01; // Ethernet PHY Basic Mode Status
const EPHY_ID1: u8 =            0x02; // Ethernet PHY Identifier Register 1
const EPHY_ID2: u8 =            0x03; // Ethernet PHY Identifier Register 2

const EPHY_REGCTL: u8 =         0x0D; // Ethernet PHY Register Control
const EPHY_ADDAR: u8 =          0x0E; // Ethernet PHY Address or Data

const EPHY_LEDCFG: u8 =         0x25; // Ethernet PHY LED Configuration

// Transmit DMA descriptor flags
const EMAC_TDES0_OWN: u32 =     0x80000000; // Indicates that the descriptor is owned by the DMA
const EMAC_TDES0_LS: u32 =      0x20000000; // Last Segment
const EMAC_TDES0_FS: u32 =      0x10000000; // First Segment
const EMAC_TDES0_TCH: u32 =     0x00100000; // Second Address Chained
const EMAC_TDES1_TBS1: u32 =    0x00001FFF; // Transmit Buffer 1 Size

// Receive DMA descriptor flags
const EMAC_RDES0_OWN: u32 =     0x80000000; // indicates that the descriptor is owned by the DMA
const EMAC_RDES0_FL: u32 =      0x3FFF0000; // Frame Length
const EMAC_RDES0_ES: u32 =      0x00008000; // Error Summary
const EMAC_RDES1_RCH: u32 =     0x00004000; // Second Address Chained
const EMAC_RDES1_RBS1: u32 =    0x00001FFF; // Receive Buffer 1 Size
const EMAC_RDES0_FS: u32 =      0x00000200; // First Descriptor
const EMAC_RDES0_LS: u32 =      0x00000100; // Last Descriptor

const ETH_DESC_U32_SIZE: usize =    8;
const ETH_TX_BUFFER_COUNT: usize =  2;
const ETH_TX_BUFFER_SIZE: usize =   1536;
const ETH_RX_BUFFER_COUNT: usize =  3;
const ETH_RX_BUFFER_SIZE: usize =   1536;

fn delay(d: u32) {
    for _ in 0..d {
        unsafe {
            asm!("
                NOP
            ");
        }
    }
}

fn phy_read(reg_addr: u8) -> u16 {
    cortex_m::interrupt::free(|cs| {
        let emac0 = tm4c129x::EMAC0.borrow(cs);

        // Make sure the MII is idle
        while emac0.miiaddr.read().miib().bit() {};

        // Tell the MAC to read the given PHY register
        unsafe {
            emac0.miiaddr.write(|w| {
                    w.cr()._100_150()
                    .mii().bits(reg_addr & 0x1F)
                    .miib().bit(true)
            });
        }

        // Wait for the read to complete
        while emac0.miiaddr.read().miib().bit() {};

        emac0.miidata.read().data().bits()
    })
}

fn phy_write(reg_addr: u8, reg_data: u16) {
    cortex_m::interrupt::free(|cs| {
        let emac0 = tm4c129x::EMAC0.borrow(cs);

        // Make sure the MII is idle
        while emac0.miiaddr.read().miib().bit() {};

        unsafe {
            emac0.miidata.write(|w| {
                w.data().bits(reg_data)
            });

            // Tell the MAC to write the given PHY register
            emac0.miiaddr.write(|w| {
                    w.cr()._100_150()
                    .mii().bits(reg_addr & 0x1F)
                    .miiw().bit(true)
                    .miib().bit(true)
            });
        }

        // Wait for the read to complete
        while emac0.miiaddr.read().miib().bit() {};
    })
}

// Writes a value to an extended PHY register in MMD address space
fn phy_write_ext(reg_addr: u8, reg_data: u16) {
    phy_write(EPHY_REGCTL, 0x001F); // set address (datasheet page 1612)
    phy_write(EPHY_ADDAR, reg_addr as u16);
    phy_write(EPHY_REGCTL, 0x401F); // set write mode
    phy_write(EPHY_ADDAR, reg_data);
}

pub struct EthernetDevice {
    tx_desc_buf: [u32; ETH_TX_BUFFER_COUNT * ETH_DESC_U32_SIZE],
    rx_desc_buf: [u32; ETH_RX_BUFFER_COUNT * ETH_DESC_U32_SIZE],
    tx_cur_desc: usize,
    rx_cur_desc: usize,
    tx_counter: u32,
    rx_counter: u32,
    tx_pkt_buf: [u8; ETH_TX_BUFFER_COUNT * ETH_TX_BUFFER_SIZE],
    rx_pkt_buf: [u8; ETH_RX_BUFFER_COUNT * ETH_RX_BUFFER_SIZE],
}

impl EthernetDevice {
    pub fn new(mac_addr: EthernetAddress) -> EthernetDevice {
        let mut device = EthernetDevice {
            tx_desc_buf: [0; ETH_TX_BUFFER_COUNT * ETH_DESC_U32_SIZE],
            rx_desc_buf: [0; ETH_RX_BUFFER_COUNT * ETH_DESC_U32_SIZE],
            tx_cur_desc: 0,
            rx_cur_desc: 0,
            tx_counter: 0,
            rx_counter: 0,
            tx_pkt_buf: [0; ETH_TX_BUFFER_COUNT * ETH_TX_BUFFER_SIZE],
            rx_pkt_buf: [0; ETH_RX_BUFFER_COUNT * ETH_RX_BUFFER_SIZE],
        };

        // Initialize TX DMA descriptors
        for x in 0..ETH_TX_BUFFER_COUNT {
            let p = x * ETH_DESC_U32_SIZE;
            let r = x * ETH_TX_BUFFER_SIZE;

            // Initialize transmit flags
            device.tx_desc_buf[p + 0] = 0;
            // Initialize transmit buffer size
            device.tx_desc_buf[p + 1] = 0;
            // Transmit buffer address
            device.tx_desc_buf[p + 2] = (&device.tx_pkt_buf[r] as *const u8) as u32;
            // Next descriptor address
            if x != ETH_TX_BUFFER_COUNT - 1 {
                device.tx_desc_buf[p + 3] = (&device.tx_desc_buf[p + ETH_DESC_U32_SIZE] as *const u32) as u32;
            } else {
                device.tx_desc_buf[p + 3] = (&device.tx_desc_buf[0] as *const u32) as u32;
            }
            // Reserved fields
            device.tx_desc_buf[p + 4] = 0;
            device.tx_desc_buf[p + 5] = 0;
            // Transmit frame time stamp
            device.tx_desc_buf[p + 6] = 0;
            device.tx_desc_buf[p + 7] = 0;
        }

        // Initialize RX DMA descriptors
        for x in 0..ETH_RX_BUFFER_COUNT {
            let p = x * ETH_DESC_U32_SIZE;
            let r = x * ETH_RX_BUFFER_SIZE;

            // The descriptor is initially owned by the DMA
            device.rx_desc_buf[p + 0] = EMAC_RDES0_OWN;
            // Use chain structure rather than ring structure
            device.rx_desc_buf[p + 1] = EMAC_RDES1_RCH  | ((ETH_RX_BUFFER_SIZE as u32) & EMAC_RDES1_RBS1);
            // Receive buffer address
            device.rx_desc_buf[p + 2] = (&device.rx_pkt_buf[r] as *const u8) as u32;
            // Next descriptor address
            if x != ETH_RX_BUFFER_COUNT - 1 {
                device.rx_desc_buf[p + 3] = (&device.rx_desc_buf[p + ETH_DESC_U32_SIZE] as *const u32) as u32;
            } else {
                device.rx_desc_buf[p + 3] = (&device.rx_desc_buf[0] as *const u32) as u32;
            }
            // Extended status
            device.rx_desc_buf[p + 4] = 0;
            // Reserved field
            device.rx_desc_buf[p + 5] = 0;
            // Transmit frame time stamp
            device.rx_desc_buf[p + 6] = 0;
            device.rx_desc_buf[p + 7] = 0;
        }

        cortex_m::interrupt::free(|cs| {
            let sysctl = tm4c129x::SYSCTL.borrow(cs);
            let emac0 = tm4c129x::EMAC0.borrow(cs);

            sysctl.rcgcemac.modify(|_, w| w.r0().bit(true)); // Bring up MAC
            sysctl.sremac.modify(|_, w| w.r0().bit(true)); // Activate MAC reset
            delay(16);
            sysctl.sremac.modify(|_, w| w.r0().bit(false)); // Dectivate MAC reset

            sysctl.rcgcephy.modify(|_, w| w.r0().bit(true)); // Bring up PHY
            sysctl.srephy.modify(|_, w| w.r0().bit(true)); // Activate PHY reset
            delay(16);
            sysctl.srephy.modify(|_, w| w.r0().bit(false)); // Dectivate PHY reset

            while !sysctl.premac.read().r0().bit() {} // Wait for the MAC to come out of reset
            while !sysctl.prephy.read().r0().bit() {} // Wait for the PHY to come out of reset
            delay(10000);

            emac0.dmabusmod.modify(|_, w| w.swr().bit(true)); // Reset MAC DMA
            while emac0.dmabusmod.read().swr().bit() {} // Wait for the MAC DMA to come out of reset
            delay(1000);

            emac0.miiaddr.write(|w| w.cr()._100_150()); // Set the MII CSR clock speed.

            // Checking PHY
            if  (phy_read(EPHY_ID1) != 0x2000) | (phy_read(EPHY_ID2) != 0xA221) {
                panic!("PHY ID error!");
            }

            // Reset PHY transceiver
            phy_write(EPHY_BMCR, 1); // Initiate MII reset
            while (phy_read(EPHY_BMCR) & 1) == 1 {}; // Wait for the reset to be completed

            // Configure PHY LEDs
            phy_write_ext(EPHY_LEDCFG, 0x0008); // LED0 Link OK/Blink on TX/RX Activity

            // Tell the PHY to start an auto-negotiation cycle
            phy_write(EPHY_BMCR, 0b00010010_00000000); // ANEN and RESTARTAN

            // Set the DMA operation mode
            emac0.dmaopmode.write(|w|
                w.rsf().bit(true) // Receive Store and Forward
                .tsf().bit(true) // Transmit Store and Forward
                .ttc()._64() // Transmit Threshold Control
                .rtc()._64() // Receive Threshold Control
            );

            // Set the bus mode register.
            emac0.dmabusmod.write(|w| unsafe {
                w.atds().bit(true)
                .aal().bit(true) // Address Aligned Beats
                .usp().bit(true) // Use Separate Programmable Burst Length ???
                .rpbl().bits(1) // RX DMA Programmable Burst Length
                .pbl().bits(1) // Programmable Burst Length
                .pr().bits(0) // Priority Ratio 1:1
            });

            // Disable all the MMC interrupts as these are enabled by default at reset.
            emac0.mmcrxim.write(|w| unsafe { w.bits(0xFFFFFFFF)});
            emac0.mmctxim.write(|w| unsafe { w.bits(0xFFFFFFFF)});

            // Set MAC configuration options
            emac0.cfg.write(|w|
                w.dupm().bit(true) // MAC operates in full-duplex mode
                .ipc().bit(true) // Checksum Offload Enable
                .prelen()._7() // 7 bytes of preamble
                .ifg()._96() // 96 bit times
                .bl()._1024() // Back-Off Limit 1024
                .ps().bit(true) // ?
            );

            // Set the maximum receive frame size
            emac0.wdogto.write(|w| unsafe {
                w.bits(0) // ??? no use watchdog
            });

            // Set the MAC address
            let mac_addr = mac_addr.0;
            emac0.addr0h.write(|w| unsafe { w.addrhi().bits(mac_addr[4] as u16 | ((mac_addr[5] as u16) << 8)) });
            emac0.addr0l.write(|w| unsafe {
                w.addrlo().bits(mac_addr[0] as u32 | ((mac_addr[1] as u32) << 8) | ((mac_addr[2] as u32) << 16) | ((mac_addr[3] as u32) << 24))
            });

            // Set MAC filtering options (?)
            emac0.framefltr.write(|w|
                w.hpf().bit(true) // Hash or Perfect Filter
                //.hmc().bit(true) // Hash Multicast ???
                .pm().bit(true) // Pass All Multicast
            );

            // Initialize hash table
            emac0.hashtbll.write(|w| unsafe { w.htl().bits(0)});
            emac0.hashtblh.write(|w| unsafe { w.hth().bits(0)});

            emac0.flowctl.write(|w| unsafe { w.bits(0)}); // Disable flow control ???

            emac0.txdladdr.write(|w| unsafe { w.bits((&device.tx_desc_buf[0] as *const u32) as u32)});
            emac0.rxdladdr.write(|w| unsafe { w.bits((&device.rx_desc_buf[0] as *const u32) as u32)});

            // Manage MAC transmission and reception
            emac0.cfg.modify(|_, w|
                w.re().bit(true) // Receiver Enable
                .te().bit(true) // Transmiter Enable
            );
            
            // Manage DMA transmission and reception
            emac0.dmaopmode.modify(|_, w|
                w.sr().bit(true) // Start Receive
                .st().bit(true) // Start Transmit
            );
        });
        device
    }

    fn release_rx_buf(&mut self) {
        self.rx_cur_desc += ETH_DESC_U32_SIZE;
        if self.rx_cur_desc >= (ETH_RX_BUFFER_COUNT * ETH_DESC_U32_SIZE) {
            self.rx_cur_desc = 0;
        }
        self.rx_desc_buf[self.rx_cur_desc + 0] = EMAC_RDES0_OWN; // release descriptor
    }
}

impl Device for EthernetDevice {
    type RxBuffer = RxBuffer;
    type TxBuffer = TxBuffer;

    fn limits(&self) -> DeviceLimits {
        let mut limits = DeviceLimits::default();
        limits.max_transmission_unit = 1500;
        limits.max_burst_size = Some(ETH_RX_BUFFER_COUNT);
        limits
    }

    fn receive(&mut self, _timestamp: u64) -> Result<Self::RxBuffer, Error> {
        if (self.rx_desc_buf[self.rx_cur_desc + 0] & EMAC_RDES0_OWN) == 0 {
            // check for the whole packet in the buffer and no any error
            if (EMAC_RDES0_FS | EMAC_RDES0_LS) == self.rx_desc_buf[self.rx_cur_desc + 0] & (EMAC_RDES0_FS | EMAC_RDES0_LS | EMAC_RDES0_ES) {
                // Retrieve the length of the frame
                let mut n = (self.rx_desc_buf[self.rx_cur_desc + 0] & EMAC_RDES0_FL) >> 16;
                // Limit the number of data to read
                if n > ETH_RX_BUFFER_SIZE as u32 { n = ETH_RX_BUFFER_SIZE as u32; }
                let sl = unsafe {
                    slice::from_raw_parts(self.rx_desc_buf[self.rx_cur_desc + 2] as * mut u8,
                                          n as usize)
                };
                Ok(RxBuffer(sl, self))
            } else {
                // Ignore invalid frame
                self.release_rx_buf();
                Err(Error::Exhausted)
            }
        } else {
            Err(Error::Exhausted) // currently no buffers to process
        }
    }

    fn transmit(&mut self, _timestamp: u64, length: usize) -> Result<Self::TxBuffer, Error> {
        // Check if the TX DMA buffer released
        if (self.tx_desc_buf[self.tx_cur_desc + 0] & EMAC_TDES0_OWN) == 0 {
            // Write the number of bytes to send
            self.tx_desc_buf[self.tx_cur_desc + 1] = length as u32 & EMAC_TDES1_TBS1;

            let sl = unsafe {
                slice::from_raw_parts_mut(self.tx_desc_buf[self.tx_cur_desc + 2] as * mut u8,
                                          ETH_TX_BUFFER_SIZE)
            };
            Ok(TxBuffer(sl, self))
        } else {
            // to do if need: Instruct the DMA to poll the receive descriptor list
            Err(Error::Exhausted)
        }
    }
}

pub struct RxBuffer(*const [u8], *mut EthernetDevice);

impl AsRef<[u8]> for RxBuffer {
    fn as_ref(&self) -> &[u8] {
        unsafe { &*self.0 }
    }
}

impl Drop for RxBuffer {
    fn drop(&mut self) {
        let mut device = unsafe { &mut *self.1 };
        device.release_rx_buf();
        device.rx_counter += 1;
    }
}

pub struct TxBuffer(*mut [u8], *mut EthernetDevice);

impl AsRef<[u8]> for TxBuffer {
    fn as_ref(&self) -> &[u8] {
        unsafe { &*self.0 }
    }
}

impl AsMut<[u8]> for TxBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *self.0 }
    }
}

impl Drop for TxBuffer {
    fn drop(&mut self) {
        let mut device = unsafe { &mut *self.1 };

        // Use chain structure rather than ring structure
        // Set LS and FS flags as the data fits in a single buffer and give the ownership of the descriptor to the DMA
        device.tx_desc_buf[device.tx_cur_desc + 0] = EMAC_TDES0_LS | EMAC_TDES0_FS | EMAC_TDES0_TCH;
        device.tx_desc_buf[device.tx_cur_desc + 0] |= EMAC_TDES0_OWN; // Set ownership for DMA here

        cortex_m::interrupt::free(|cs| {
            let emac0 = tm4c129x::EMAC0.borrow(cs);
            // Clear TU flag to resume processing
            emac0.dmaris.write(|w| w.tu().bit(true));
            // Instruct the DMA to poll the transmit descriptor list
            unsafe { emac0.txpolld.write(|w| w.tpd().bits(0)); }
        });

        // Calculate next DMA descriptor offset
        let mut tx_next_desc = device.tx_cur_desc + ETH_DESC_U32_SIZE;
        if tx_next_desc >= (ETH_TX_BUFFER_COUNT * ETH_DESC_U32_SIZE) {
            tx_next_desc = 0;
        }
        device.tx_cur_desc = tx_next_desc;

        device.tx_counter += 1;
    }
}
