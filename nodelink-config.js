import defaultConfig from './config.default.js'

const config = {
  ...defaultConfig,
  server: {
    ...defaultConfig.server,
    port: 3000,
    password: 'youshallnotpass',
    bufferDurationMs: 10000,
    frameBufferDurationMs: 25000,
    trackStuckThresholdMs: 60000
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
      allowItag: [251, 140, 250, 249],
      clients: {
        ...defaultConfig.sources.youtube.clients,
        search: ['Web', 'Android', 'IOS'],
        playback: [
          'IOS',
          'TVCast',
          'WebEmbedded',
          'WebParentTools',
          'AndroidVR',
          'Web'
        ]
      }
    }
  },
  audio: {
    ...defaultConfig.audio,
    quality: 'high',
    encryption: 'aead_xchacha20_poly1305_rtpsize',
    resamplingQuality: 'fastest',
    lookaheadMs: 250,
    crossfade: {
      enabled: false,
      duration: 0,
      curve: 'sinusoidal',
      mode: 'preload',
      minBufferMs: 5000,
      bufferMs: 10000
    }
  }
}

export default config
