target remote :3333
monitor arm semihosting enable
load
tbreak cortex_m_rt::reset_handler
monitor reset halt
continue
