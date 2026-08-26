export interface SmartTrainStatus {
  running: boolean
  text?: string
}

let status: SmartTrainStatus = { running: false }
const listeners = new Set<() => void>()

export function setSmartTrainStatus(next: SmartTrainStatus) {
  status = next
  listeners.forEach((listener) => listener())
}

export function getSmartTrainStatus(): SmartTrainStatus {
  return status
}

export function subscribeSmartTrainStatus(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}
