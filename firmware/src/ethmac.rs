use core::{slice, cmp};
use cortex_m::{self, asm::delay};
use tm4c129x;
use smoltcp::Result;
use smoltcp::time::Instant;
use smoltcp::wire::EthernetAddress;
use smoltcp::phy;

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
#[allow(dead_code)]
const EMAC_TDES1_TBS1: u32 =    0x00001FFF; // Transmit Buffer 1 Size

// Receive DMA descriptor flags
const EMAC_RDES0_OWN: u32 =     0x80000000; // indicates that the descriptor is owned by the DMA
const EMAC_RDES0_FL: u32 =      0x3FFF0000; // Frame Length
const EMAC_RDES0_ES: u32 =      0x00008000; // Error Summary
const EMAC_RDES0_FS: u32 =      0x00000200; // First Descriptor
const EMAC_RDES0_LS: u32 =      0x00000100; // Last Descriptor
const EMAC_RDES1_RCH: u32 =     0x00004000; // Second Address Chained
const EMAC_RDES1_RBS1: u32 =    0x00001FFF; // Receive Buffer 1 Size

const ETH_DESC_U32_SIZE: usize =    8;
const ETH_TX_BUFFER_COUNT: usize =  2;
const ETH_TX_BUFFER_SIZE: usize =   1536;
const ETH_RX_BUFFER_COUNT: usize =  3;
const ETH_RX_BUFFER_SIZE: usize =   1536;

fn phy_read(reg_addr: u8) -> u16 {
    cortex_m::interrupt::free(|_cs| {
        let emac0 = unsafe { &*tm4c129x::EMAC0::ptr() };

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
    cortex_m::interrupt::free(|_cs| {
        let emac0 = unsafe { &*tm4c129x::EMAC0::ptr() };

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

struct RxRing {
    desc_buf: [u32; ETH_RX_BUFFER_COUNT * ETH_DESC_U32_SIZE],
    cur_desc: usize,
    counter: u32,
    pkt_buf: [u8; ETH_RX_BUFFER_COUNT * ETH_RX_BUFFER_SIZE],
}

impl RxRing {
    fn new() -> RxRing {
        RxRing {
            desc_buf: [0; ETH_RX_BUFFER_COUNT * ETH_DESC_U32_SIZE],
            cur_desc: 0,
            counter: 0,
            pkt_buf: [0; ETH_RX_BUFFER_COUNT * ETH_RX_BUFFER_SIZE],
        }
    }

    fn init(&mut self) {
        // Initialize RX DMA descriptors
        for x in 0..ETH_RX_BUFFER_COUNT {
            let p = x * ETH_DESC_U32_SIZE;
            let r = x * ETH_RX_BUFFER_SIZE;

            // The descriptor is initially owned by the DMA
            self.desc_buf[p + 0] = EMAC_RDES0_OWN;
            // Use chain structure rather than ring structure
            self.desc_buf[p + 1] =
                EMAC_RDES1_RCH | ((ETH_RX_BUFFER_SIZE as u32) & EMAC_RDES1_RBS1);
            // Receive buffer address
            self.desc_buf[p + 2] = (&self.pkt_buf[r] as *const u8) as u32;
            // Next descriptor address
            if x != ETH_RX_BUFFER_COUNT - 1 {
                self.desc_buf[p + 3] =
                    (&self.desc_buf[p + ETH_DESC_U32_SIZE] as *const u32) as u32;
            } else {
                self.desc_buf[p + 3] =
                    (&self.desc_buf[0] as *const u32) as u32;
            }
            // Extended status
            self.desc_buf[p + 4] = 0;
            // Reserved field
            self.desc_buf[p + 5] = 0;
            // Transmit frame time stamp
            self.desc_buf[p + 6] = 0;
            self.desc_buf[p + 7] = 0;
        }
    }

    fn buf_owned(&self) -> bool {
        self.desc_buf[self.cur_desc + 0] & EMAC_RDES0_OWN == 0
    }

    fn buf_valid(&self) -> bool {
        self.desc_buf[self.cur_desc + 0] &
            (EMAC_RDES0_FS | EMAC_RDES0_LS | EMAC_RDES0_ES) ==
            (EMAC_RDES0_FS | EMAC_RDES0_LS)
    }

    unsafe fn buf_as_slice<'a>(&self) -> &'a [u8] {
        let len  = (self.desc_buf[self.cur_desc + 0] & EMAC_RDES0_FL) >> 16;
        let len  = cmp::min(len as usize, ETH_RX_BUFFER_SIZE);
        let addr = self.desc_buf[self.cur_desc + 2] as *const u8;
        slice::from_raw_parts(addr, len)
    }

    fn buf_release(&mut self) {
        self.cur_desc += ETH_DESC_U32_SIZE;
        if self.cur_desc == self.desc_buf.len() {
            self.cur_desc = 0;
        }
        self.counter += 1;

        self.desc_buf[self.cur_desc + 0] = EMAC_RDES0_OWN;
    }
}

struct TxRing {
    desc_buf: [u32; ETH_TX_BUFFER_COUNT * ETH_DESC_U32_SIZE],
    cur_desc: usize,
    counter: u32,
    pkt_buf: [u8; ETH_TX_BUFFER_COUNT * ETH_TX_BUFFER_SIZE],
}

impl TxRing {
    fn new() -> TxRing {
        TxRing {
            desc_buf: [0; ETH_TX_BUFFER_COUNT * ETH_DESC_U32_SIZE],
            cur_desc: 0,
            counter: 0,
            pkt_buf: [0; ETH_TX_BUFFER_COUNT * ETH_TX_BUFFER_SIZE],
        }
    }

    fn init(&mut self) {
        // Initialize TX DMA descriptors
        for x in 0..ETH_TX_BUFFER_COUNT {
            let p = x * ETH_DESC_U32_SIZE;
            let r = x * ETH_TX_BUFFER_SIZE;

            // Initialize transmit flags
            self.desc_buf[p + 0] = 0;
            // Initialize transmit buffer size
            self.desc_buf[p + 1] = 0;
            // Transmit buffer address
            self.desc_buf[p + 2] = (&self.pkt_buf[r] as *const u8) as u32;
            // Next descriptor address
            if x != ETH_TX_BUFFER_COUNT - 1 {
                self.desc_buf[p + 3] =
                    (&self.desc_buf[p + ETH_DESC_U32_SIZE] as *const u32) as u32;
            } else {
                self.desc_buf[p + 3] =
                    (&self.desc_buf[0] as *const u32) as u32;
            }
            // Reserved fields
            self.desc_buf[p + 4] = 0;
            self.desc_buf[p + 5] = 0;
            // Transmit frame time stamp
            self.desc_buf[p + 6] = 0;
            self.desc_buf[p + 7] = 0;
        }
    }

    fn buf_owned(&self) -> bool {
        self.desc_buf[self.cur_desc + 0] & EMAC_TDES0_OWN == 0
    }

    unsafe fn buf_as_slice<'a>(&mut self, len: usize) -> &'a mut [u8] {
        let len = cmp::min(len, ETH_TX_BUFFER_SIZE);
        self.desc_buf[self.cur_desc + 1] = len as u32;
        let addr = self.desc_buf[self.cur_desc + 2] as *mut u8;
        slice::from_raw_parts_mut(addr, len)
    }

    fn buf_release(&mut self) {
        self.desc_buf[self.cur_desc + 0] =
            EMAC_TDES0_OWN | EMAC_TDES0_LS | EMAC_TDES0_FS | EMAC_TDES0_TCH;

        cortex_m::interrupt::free(|_cs| {
            let emac0 = unsafe { &*tm4c129x::EMAC0::ptr() };
            // Clear TU flag to resume processing
            emac0.dmaris.write(|w| w.tu().bit(true));
            // Instruct the DMA to poll the transmit descriptor list
            unsafe { emac0.txpolld.write(|w| w.tpd().bits(0)); }
        });

        self.cur_desc += ETH_DESC_U32_SIZE;
        if self.cur_desc == self.desc_buf.len() {
            self.cur_desc = 0;
        }
        self.counter += 1;
    }
}

pub struct Device {
    rx: RxRing,
    tx: TxRing,
}

impl Device {
    pub fn new() -> Device {
        Device {
            rx: RxRing::new(),
            tx: TxRing::new(),
        }
    }

    // After `init` is called, `Device` shall not be moved.
    pub unsafe fn init(&mut self, mac: EthernetAddress) {
        self.rx.init();
        self.tx.init();

        cortex_m::interrupt::free(|_cs| {
            let sysctl = &*tm4c129x::SYSCTL::ptr();
            let emac0 = &*tm4c129x::EMAC0::ptr();

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
            emac0.dmabusmod.write(|w|
                w.atds().bit(true)
                 .aal().bit(true) // Address Aligned Beats
                 .usp().bit(true) // Use Separate Programmable Burst Length ???
                 .rpbl().bits(1) // RX DMA Programmable Burst Length
                 .pbl().bits(1) // Programmable Burst Length
                 .pr().bits(0) // Priority Ratio 1:1
            );

            // Disable all the MMC interrupts as these are enabled by default at reset.
            emac0.mmcrxim.write(|w| w.bits(0xFFFFFFFF));
            emac0.mmctxim.write(|w| w.bits(0xFFFFFFFF));

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
            emac0.wdogto.write(|w|
                w.bits(0) // ??? no use watchdog
            );

            // Set the MAC address
            emac0.addr0l.write(|w|
                w.addrlo().bits(  mac.0[0] as u32 |
                                ((mac.0[1] as u32) <<  8) |
                                ((mac.0[2] as u32) << 16) |
                                ((mac.0[3] as u32) << 24))
            );
            emac0.addr0h.write(|w|
                w.addrhi().bits(  mac.0[4] as u16 |
                                ((mac.0[5] as u16) << 8))
            );

            // Set MAC filtering options (?)
            emac0.framefltr.write(|w|
                w.hpf().bit(true) // Hash or Perfect Filter
                //.hmc().bit(true) // Hash Multicast ???
                 .pm().bit(true) // Pass All Multicast
            );

            // Initialize hash table
            emac0.hashtbll.write(|w| w.htl().bits(0));
            emac0.hashtblh.write(|w| w.hth().bits(0));

            emac0.flowctl.write(|w| w.bits(0)); // Disable flow control ???

            emac0.txdladdr.write(|w| /*unsafe*/ {
                w.bits((&mut self.tx.desc_buf[0] as *mut u32) as u32)
            });
            emac0.rxdladdr.write(|w| /*unsafe*/ {
                w.bits((&mut self.rx.desc_buf[0] as *mut u32) as u32)
            });

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
    }
}

impl<'a, 'b> phy::Device<'a> for &'b mut Device {
    type RxToken = RxToken<'a>;
    type TxToken = TxToken<'a>;

    fn capabilities(&self) -> phy::DeviceCapabilities {
        let mut capabilities = phy::DeviceCapabilities::default();
        capabilities.max_transmission_unit = 1500;
        capabilities.max_burst_size = Some(ETH_RX_BUFFER_COUNT);
        capabilities
    }

    fn receive(&mut self) -> Option<(RxToken, TxToken)> {
        // Skip all queued packets with errors.
        while self.rx.buf_owned() && !self.rx.buf_valid() {
            self.rx.buf_release()
        }

        if !(self.rx.buf_owned() && self.tx.buf_owned()) {
            return None
        }

        Some((RxToken(&mut self.rx), TxToken(&mut self.tx)))
    }

    fn transmit(&mut self) -> Option<TxToken> {
        if !self.tx.buf_owned() {
            return None
        }

        Some(TxToken(&mut self.tx))
    }
}

pub struct RxToken<'a>(&'a mut RxRing);

impl<'a> phy::RxToken for RxToken<'a> {
    fn consume<R, F>(self, _timestamp: Instant, f: F) -> Result<R>
            where F: FnOnce(&[u8]) -> Result<R> {
        let result = f(unsafe { self.0.buf_as_slice() });
        self.0.buf_release();
        result
    }
}

pub struct TxToken<'a>(&'a mut TxRing);

impl<'a> phy::TxToken for TxToken<'a> {
    fn consume<R, F>(self, _timestamp: Instant, len: usize, f: F) -> Result<R>
            where F: FnOnce(&mut [u8]) -> Result<R> {
        let result = f(unsafe { self.0.buf_as_slice(len) });
        self.0.buf_release();
        result
    }
}
