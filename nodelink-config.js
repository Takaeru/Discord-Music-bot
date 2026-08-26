import defaultConfig from './config.default.js'

const config = {
  ...defaultConfig,
  server: {
    ...defaultConfig.server,
    port: 3000,
    password: 'youshallnotpass',
    bufferDurationMs: 10000,
    frameBufferDurationMs: 20000,
    trackStuckThresholdMs: 50000
  },
  cluster: {
    ...defaultConfig.cluster,
    enabled: true,
    workers: 1,
    minWorkers: 1,
    hibernation: {
      enabled: false,
      timeoutMs: 1200000
    },
    specializedSourceWorker: {
      ...defaultConfig.cluster?.specializedSourceWorker,
      enabled: true,
      count: 1,
      microWorkers: 2,
      silentLogs: true
    }
  },
  sources: {
    ...defaultConfig.sources,
    youtube: {
      ...defaultConfig.sources.youtube,
      enabled: true,
      allowItag: [251, 140, 250, 249],
      clients: {
        ...defaultConfig.sources.youtube.clients,
        search: ['Web', 'Android', 'TVCast'],
        playback: [
          'TVCast',
          'IOS',
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
    lookaheadMs: 50,
    crossfade: {
      enabled: false,
      duration: 0,
      curve: 'sinusoidal',
      mode: 'preload',
      minBufferMs: 2000,
      bufferMs: 5000
    }
  }
}

export default config
