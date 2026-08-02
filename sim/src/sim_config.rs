#[derive(Clone, Copy, Debug)]
pub struct SimErrorModelConfig {
    /// Spectral power density in dBm/MHz (P_{TX, PSD})
    pub power_spectral_density_dbm_mhz: f64,
    /// UWB bandwidth in Hz (B)
    pub bandwidth_hz: f64,
    /// G_{TX}
    pub tx_antenna_gain_dbi: f64,
    /// G_{RX}
    pub rx_antenna_gain_dbi: f64,
    /// Standing Wave Ration of transmitting antenna (SWR_{TX})
    pub rx_swr: f64,
    /// Standing Wave Ration of receiving antenna (SWR_{RX})
    pub tx_swr: f64,
    /// Receiver noice figure (NF = Rauschzahl)
    pub noise_figure_db: f64,
    /// Additional losses in dB (L_{other})
    pub other_losses_db: f64,
    /// Minimum hardware error in m (sigma_{floor})
    pub floor_error_m: f64,
    /// Reference distance standard deviation in m (sigma_{ref})
    pub reference_error_m: f64,
    /// Reference SNR in dB (SNR_{ref})
    pub reference_snr_db: f64,
    /// Reference preamble length
    pub reference_preamble_length: f64,

    // Pre-calculated constants
    /// Total transmission power in dBm (P_{TX, total})
    pub p_tx_total: f64,
    /// SWR loss of trnasmitting antenna (L_{SWR, TX})
    pub l_swr_tx: f64,
    /// SWR loss of receiving antenna (L_{SWR, RX})
    pub l_swr_rx: f64,
    /// Receiver thermal noice floor in dBm (P_{noise})
    pub p_noise: f64,
    /// Base received power with out free space loss/ Log Distance Path Model
    pub base_rx_power: f64,
}

impl Default for SimErrorModelConfig {
    fn default() -> Self {
        let psd = -41.3;
        let bandwidth_hz = 499_200_000.0;
        let tx_antenna_gain_dbi = 0.0;
        let rx_antenna_gain_dbi = 0.0;
        let tx_swr = 1.5;
        let rx_swr = 1.5;
        let noise_figure_db = 6.0;
        let other_losses_db = 0.0;

        let mut config = Self {
            power_spectral_density_dbm_mhz: psd,
            bandwidth_hz,
            tx_antenna_gain_dbi,
            rx_antenna_gain_dbi,
            tx_swr,
            rx_swr,
            noise_figure_db,
            other_losses_db,
            floor_error_m: 0.025,
            reference_error_m: 0.05,
            reference_snr_db: 20.0,
            reference_preamble_length: 64.0,

            p_tx_total: 0.0,
            l_swr_tx: 0.0,
            l_swr_rx: 0.0,
            p_noise: 0.0,
            base_rx_power: 0.0,
        };

        config.calc_constants(
            psd,
            bandwidth_hz,
            tx_antenna_gain_dbi,
            rx_antenna_gain_dbi,
            tx_swr,
            rx_swr,
            noise_figure_db,
            other_losses_db,
        );
        config
    }
}

impl SimErrorModelConfig {
    pub fn new(
        psd: f64,
        bandwidth_hz: f64,
        tx_antenna_gain_dbi: f64,
        rx_antenna_gain_dbi: f64,
        tx_swr: f64,
        rx_swr: f64,
        noise_figure_db: f64,
        other_losses_db: f64,
        floor_error_m: f64,
        reference_error_m: f64,
        reference_snr_db: f64,
        reference_preamble_length: f64,
    ) -> Self {
        let mut config = Self {
            power_spectral_density_dbm_mhz: psd,
            bandwidth_hz,
            tx_antenna_gain_dbi,
            rx_antenna_gain_dbi,
            tx_swr,
            rx_swr,
            noise_figure_db,
            other_losses_db,
            floor_error_m,
            reference_error_m,
            reference_snr_db,
            reference_preamble_length,

            p_tx_total: 0.0,
            l_swr_tx: 0.0,
            l_swr_rx: 0.0,
            p_noise: 0.0,
            base_rx_power: 0.0,
        };

        config.calc_constants(
            psd,
            bandwidth_hz,
            tx_antenna_gain_dbi,
            rx_antenna_gain_dbi,
            tx_swr,
            rx_swr,
            noise_figure_db,
            other_losses_db,
        );
        config
    }
    fn calc_constants(
        &mut self,
        psd: f64,
        bandwidth_hz: f64,
        tx_antenna_gain_dbi: f64,
        rx_antenna_gain_dbi: f64,
        tx_swr: f64,
        rx_swr: f64,
        noise_figure_db: f64,
        other_losses_db: f64,
    ) {
        let bandwdth_mhz = bandwidth_hz / 1_000_000.0;
        self.p_tx_total = psd + 10 * bandwdth_mhz.log10();

        let gamma_tx = (tx_swr - 1.0) / (tx_swr + 1.0);
        self.l_swr_tx = -10.0 * (1.0 - gamma_tx.powi(2)).log10();

        let gamma_rx = (rx_swr - 1.0) / (rx_swr + 1.0);
        self.l_swr_rx = -10.0 * (1.0 - gamma_rx.powi(2)).log10();

        self.p_noise = -174.0 + 10.0 * bandwidth_hz.log10() + noise_figure_db;

        self.base_rx_power = self.p_tx_total + tx_antenna_gain_dbi + rx_antenna_gain_dbi
            - self.l_swr_rx
            - self.l_swr_tx
            - other_losses_db;
    }
}
