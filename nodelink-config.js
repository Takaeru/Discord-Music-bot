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
      targetItag: 251,
      allowItag: [251, 250, 249],
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
    },
    // Disable background probe sources to save CPU and RAM
    monochrome: { ...defaultConfig.sources.monochrome, enabled: false },
    instagram: { ...defaultConfig.sources.instagram, enabled: false },
    twitter: { ...defaultConfig.sources.twitter, enabled: false },
    tiktok: { ...defaultConfig.sources.tiktok, enabled: false },
    reddit: { ...defaultConfig.sources.reddit, enabled: false },
    tumblr: { ...defaultConfig.sources.tumblr, enabled: false },
    bilibili: { ...defaultConfig.sources.bilibili, enabled: false },
    nicovideo: { ...defaultConfig.sources.nicovideo, enabled: false },
    flowery: { ...defaultConfig.sources.flowery, enabled: false },
    lazypytts: { ...defaultConfig.sources.lazypytts, enabled: false },
    'google-tts': { ...defaultConfig.sources['google-tts'], enabled: false },
    pipertts: { ...defaultConfig.sources.pipertts, enabled: false },
    pandora: { ...defaultConfig.sources.pandora, enabled: false },
    tidal: { ...defaultConfig.sources.tidal, enabled: false },
    qobuz: { ...defaultConfig.sources.qobuz, enabled: false },
    lastfm: { ...defaultConfig.sources.lastfm, enabled: false },
    netease: { ...defaultConfig.sources.netease, enabled: false },
    letrasmus: { ...defaultConfig.sources.letrasmus, enabled: false },
    yandexmusic: { ...defaultConfig.sources.yandexmusic, enabled: false },
    googledrive: { ...defaultConfig.sources.googledrive, enabled: false },
    kwai: { ...defaultConfig.sources.kwai, enabled: false },
    audius: { ...defaultConfig.sources.audius, enabled: false }
  },
  audio: {
    ...defaultConfig.audio,
    quality: 'medium',
    encryption: 'aead_xchacha20_poly1305_rtpsize',
    resamplingQuality: 'zero',
    lookaheadMs: 5
  }
}

export default config
