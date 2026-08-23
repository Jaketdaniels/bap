const { EventEmitter } = require("node:events");

class EventBus extends EventEmitter {
  publish(channel, payload) {
    this.emit(channel, payload);
  }
  subscribe(channel, handler) {
    this.on(channel, handler);
    return () => this.off(channel, handler);
  }
}
module.exports = { EventBus };
