import { TrafficDataSampler, isSameTrafficData } from '../utils/traffic-sampler'

interface WorkerScope {
  postMessage: (message: unknown) => void
  onmessage: ((event: MessageEvent<TrafficWorkerRequestMessage>) => void) | null
  setTimeout: typeof setTimeout
  clearTimeout: typeof clearTimeout
  setInterval: typeof setInterval
  clearInterval: typeof clearInterval
}

const ctx: WorkerScope = self as unknown as WorkerScope

const DEFAULT_CONFIG: ISamplingConfig & {
  snapshotIntervalMs: number
  defaultRangeMinutes: number
} = {
  rawDataMinutes: 10,
  compressedDataMinutes: 60,
  compressionRatio: 5,
  snapshotIntervalMs: 250,
  defaultRangeMinutes: 10,
}

// 'init' is the first message posted by the client, so these are always
// assigned before first use.
let config = { ...DEFAULT_CONFIG }
let sampler = new TrafficDataSampler(config)
let currentRangeMinutes = config.defaultRangeMinutes
let throttleTimer: ReturnType<typeof setTimeout> | null = null
let agingTimer: ReturnType<typeof setInterval> | null = null
let lastTimestamp: number | undefined
let lastBroadcast: ITrafficDataPoint[] = []

const broadcastSnapshot = () => {
  const dataPoints = sampler.getDataForTimeRange(currentRangeMinutes)
  const samplerStats = sampler.getStats()

  const message: ITrafficWorkerSnapshotMessage = {
    type: 'snapshot',
    dataPoints,
    samplerStats,
    lastTimestamp,
  }

  ctx.postMessage(message)
  lastBroadcast = dataPoints
}

const scheduleSnapshot = () => {
  if (throttleTimer !== null) return
  throttleTimer = ctx.setTimeout(() => {
    throttleTimer = null
    broadcastSnapshot()
  }, config.snapshotIntervalMs)
}

ctx.onmessage = (event: MessageEvent<TrafficWorkerRequestMessage>) => {
  const message = event.data

  switch (message.type) {
    case 'init': {
      config = { ...message.config }
      sampler = new TrafficDataSampler(config)
      currentRangeMinutes = message.config.defaultRangeMinutes
      // Re-broadcast when points age out of the range, not on every tick;
      // no handle cleanup needed, worker teardown (terminate) drops timers.
      if (agingTimer !== null) ctx.clearInterval(agingTimer)
      agingTimer = ctx.setInterval(() => {
        if (throttleTimer !== null) return
        const slice = sampler.getDataForTimeRange(currentRangeMinutes)
        if (!isSameTrafficData(lastBroadcast, slice)) {
          broadcastSnapshot()
        }
      }, 1000)
      broadcastSnapshot()
      break
    }
    case 'append': {
      const timestamp = message.payload.timestamp ?? Date.now()
      const dataPoint: ITrafficDataPoint = {
        up: message.payload.up || 0,
        down: message.payload.down || 0,
        timestamp,
      }

      lastTimestamp = timestamp
      sampler.addDataPoint(dataPoint)
      scheduleSnapshot()
      break
    }
    case 'setRange': {
      if (currentRangeMinutes !== message.minutes) {
        currentRangeMinutes = message.minutes
        broadcastSnapshot()
      }
      break
    }
    case 'requestSnapshot': {
      broadcastSnapshot()
      break
    }
    default:
      break
  }
}
