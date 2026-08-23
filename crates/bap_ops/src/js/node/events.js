// node:events - EventEmitter with Node-compatible method names.
__bapNodeModules.events = (() => {
  const DEFAULT_MAX_LISTENERS = 10;

  class EventEmitter {
    #events = new Map();
    #maxListeners = DEFAULT_MAX_LISTENERS;

    static get defaultMaxListeners() {
      return DEFAULT_MAX_LISTENERS;
    }

    setMaxListeners(count) {
      this.#maxListeners = count;
      return this;
    }

    getMaxListeners() {
      return this.#maxListeners;
    }

    on(eventName, listener) {
      if (typeof listener !== "function") throw new TypeError("listener must be a function");
      const existing = this.#events.get(eventName);
      if (existing) existing.push(listener);
      else this.#events.set(eventName, [listener]);
      this.emit("newListener", eventName, listener);
      return this;
    }

    addListener = this.on;
    off = this.removeListener;
    prependListener = this.on;

    once(eventName, listener) {
      const wrapped = (...args) => {
        this.removeListener(eventName, wrapped);
        listener(...args);
      };
      wrapped.__bapOriginal = listener;
      return this.on(eventName, wrapped);
    }

    removeListener(eventName, listener) {
      const existing = this.#events.get(eventName);
      if (!existing) return this;
      const index = existing.findIndex(
        (candidate) => candidate === listener || candidate.__bapOriginal === listener,
      );
      if (index >= 0) {
        existing.splice(index, 1);
        if (existing.length === 0) this.#events.delete(eventName);
      }
      return this;
    }

    removeAllListeners(eventName) {
      if (eventName === undefined) this.#events.clear();
      else this.#events.delete(eventName);
      return this;
    }

    emit(eventName, ...args) {
      const listeners = this.#events.get(eventName);
      if (!listeners || listeners.length === 0) {
        if (eventName === "error") throw args[0];
        return false;
      }
      for (const listener of [...listeners]) {
        listener(...args);
      }
      return true;
    }

    listeners(eventName) {
      return [...(this.#events.get(eventName) ?? [])];
    }

    listenerCount(eventName) {
      return (this.#events.get(eventName) ?? []).length;
    }

    eventNames() {
      return [...this.#events.keys()].filter((name) => name !== "newListener");
    }
  }

  return Object.freeze({
    EventEmitter,
    defaultMaxListeners: DEFAULT_MAX_LISTENERS,
    once: (emitter, eventName) =>
      new Promise((resolve) => {
        const handler = (...args) => resolve(args.length <= 1 ? args[0] : args);
        emitter.once(eventName, handler);
      }),
  });
})();
