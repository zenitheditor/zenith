// Errors the Engine interface raises (see `index.js`).

/** The engine cannot be reached (network down, server gone). */
export class EngineUnavailable extends Error {
  constructor(message) {
    super(message);
    this.name = "EngineUnavailable";
    this.code = "edit.unreachable";
  }
}

/** A request the engine host refused, with its stable code. */
export class EngineError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "EngineError";
    this.code = code;
  }
}
