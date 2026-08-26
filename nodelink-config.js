import defaultConfig from './config.default.js'

const config = {
  ...defaultConfig,
  server: {
    ...defaultConfig.server,
    port: 3000,
    password: 'youshallnotpass'
  },
  cluster: {
    ...defaultConfig.cluster,
    enabled: false
  },
  sources: {
    ...defaultConfig.sources,
    youtube: {
      ...defaultConfig.sources.youtube,
      enabled: true,
      clients: {
        ...defaultConfig.sources.youtube.clients,
        search: ['Web', 'Android'],
        playback: [
          'TVCast',
          'WebEmbedded',
          'WebParentTools',
          'AndroidVR',
          'IOS',
          'Web'
        ]
      }
    }
  },
  audio: {
    ...defaultConfig.audio,
    quality: 'high',
    encryption: 'aead_aes256_gcm_rtpsize',
    resamplingQuality: 'best',
    lookaheadMs: 5
  }
}

export default config
