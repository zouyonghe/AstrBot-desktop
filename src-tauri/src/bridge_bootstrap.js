(() => {
  const existingTrayRestartState = window.__astrbotDesktopTrayRestartState;
  if (
    window.astrbotDesktop &&
    window.astrbotDesktop.__tauriBridge === true &&
    typeof window.astrbotDesktop.onTrayRestartBackend === 'function' &&
    typeof existingTrayRestartState?.unlistenTrayRestartBackendEvent === 'function'
  ) {
    return;
  }

  const invoke = window.__TAURI_INTERNALS__?.invoke;
  const transformCallback = window.__TAURI_INTERNALS__?.transformCallback;
  const tauriEvent = window.__TAURI_INTERNALS__?.event ?? window.__TAURI__?.event;
  if (typeof invoke !== 'function') return;

  const BRIDGE_COMMANDS = Object.freeze({
    IS_DESKTOP_RUNTIME: 'desktop_bridge_is_desktop_runtime',
    GET_BACKEND_STATE: 'desktop_bridge_get_backend_state',
    SET_AUTH_TOKEN: 'desktop_bridge_set_auth_token',
    RESTART_BACKEND: 'desktop_bridge_restart_backend',
    STOP_BACKEND: 'desktop_bridge_stop_backend',
    OPEN_EXTERNAL_URL: 'desktop_bridge_open_external_url',
  });
  const TRAY_RESTART_BACKEND_EVENT = '{TRAY_RESTART_BACKEND_EVENT}';

  const invokeBridge = async (command, payload = {}) => {
    try {
      return await invoke(command, payload);
    } catch (error) {
      return { ok: false, reason: String(error) };
    }
  };

  const createLegacyEventListener = async (eventName, handler) => {
    if (typeof transformCallback !== 'function') {
      throw new Error(
        'No supported Tauri event listener API: expected tauriEvent.listen or __TAURI_INTERNALS__.invoke + transformCallback'
      );
    }

    let eventId;
    try {
      eventId = await invoke('plugin:event|listen', {
        event: eventName,
        target: { kind: 'Any' },
        handler: transformCallback(handler),
      });
    } catch (error) {
      throw new Error(`plugin:event|listen failed: ${String(error)}`);
    }

    return async () => {
      try {
        window.__TAURI_EVENT_PLUGIN_INTERNALS__?.unregisterListener?.(eventName, eventId);
      } catch {}
      try {
        await invoke('plugin:event|unlisten', {
          event: eventName,
          eventId,
        });
      } catch {}
    };
  };

  const createEventListener = async (eventName, handler) => {
    if (typeof tauriEvent?.listen === 'function') {
      return tauriEvent.listen(eventName, handler);
    }
    return createLegacyEventListener(eventName, handler);
  };

  const trayRestartState =
    window.__astrbotDesktopTrayRestartState ||
    (window.__astrbotDesktopTrayRestartState = {
      handlers: new Set(),
      pending: 0,
      lastToken: 0,
      unlistenTrayRestartBackendEvent: null
    });
  if (
    typeof trayRestartState.lastToken !== 'number' ||
    !Number.isFinite(trayRestartState.lastToken)
  ) {
    trayRestartState.lastToken = 0;
  }
  if (typeof trayRestartState.unlistenTrayRestartBackendEvent === 'undefined') {
    trayRestartState.unlistenTrayRestartBackendEvent = null;
  }

  const shouldEmitForToken = (token) => {
    const numericToken = Number(token);
    if (Number.isFinite(numericToken) && numericToken > 0) {
      if (numericToken <= trayRestartState.lastToken) return false;
      trayRestartState.lastToken = numericToken;
      return true;
    } else {
      trayRestartState.lastToken += 1;
      return true;
    }
  };

  const emitTrayRestart = (token = null) => {
    if (!shouldEmitForToken(token)) return;

    if (trayRestartState.handlers.size === 0) {
      trayRestartState.pending = Number(trayRestartState.pending || 0) + 1;
      return;
    }
    for (const handler of trayRestartState.handlers) {
      try {
        handler();
      } catch {}
    }
  };

  const onTrayRestartBackend = (callback) => {
    if (typeof callback !== 'function') return () => {};
    const handler = () => callback();
    trayRestartState.handlers.add(handler);
    while (trayRestartState.pending > 0) {
      trayRestartState.pending -= 1;
      handler();
    }
    return () => {
      trayRestartState.handlers.delete(handler);
    };
  };

  const listenToTrayRestartBackendEvent = async () => {
    if (typeof trayRestartState.unlistenTrayRestartBackendEvent === 'function') return;
    try {
      const unlisten = await createEventListener(TRAY_RESTART_BACKEND_EVENT, (event) => {
        emitTrayRestart(event?.payload);
      });
      if (typeof unlisten === 'function') {
        trayRestartState.unlistenTrayRestartBackendEvent = unlisten;
      }
    } catch (error) {
      console.warn('Failed to listen for tray restart backend event', error);
    }
  };

  const getStoredAuthToken = () => {
    try {
      const token = window.localStorage?.getItem('token');
      return typeof token === 'string' && token ? token : null;
    } catch {
      return null;
    }
  };

  const syncAuthToken = (token = getStoredAuthToken()) =>
    invokeBridge(BRIDGE_COMMANDS.SET_AUTH_TOKEN, {
      authToken: typeof token === 'string' && token ? token : null
    });

  const IS_DEV =
    (typeof process !== 'undefined' &&
      process.env &&
      process.env.NODE_ENV !== 'production') ||
    (typeof __DEV__ !== 'undefined' && __DEV__ === true);
  const devWarn = (...args) => {
    if (!IS_DEV) return;
    if (typeof console !== 'undefined' && typeof console.warn === 'function') {
      console.warn(...args);
    }
  };
  const safePatch = (label, patchFn) => {
    try {
      patchFn();
    } catch (error) {
      if (!(error instanceof TypeError)) {
        throw error;
      }
      devWarn(`astrbotDesktop: failed to patch ${label}`, error);
    }
  };
  const warnExternalUrlBridgeError = (phase, url, error) => {
    devWarn('[astrbotDesktop] openExternalUrl bridge failure', {
      phase,
      url,
      error,
    });
  };

  const normalizeExternalHttpUrl = (rawUrl) => {
    if (rawUrl instanceof URL) {
      if (rawUrl.protocol !== 'http:' && rawUrl.protocol !== 'https:') return null;
      return rawUrl;
    }

    if (typeof rawUrl !== 'string') return null;

    const normalized = rawUrl.trim();
    if (!normalized) return null;

    try {
      const url = new URL(normalized, window.location.href);
      if (url.protocol !== 'http:' && url.protocol !== 'https:') return null;
      return url;
    } catch {
      return null;
    }
  };

  const openExternalUrl = (rawUrl) => {
    const url = normalizeExternalHttpUrl(rawUrl);
    if (!url) return false;
    if (url.origin === window.location.origin) {
      return false;
    }

    const bridgeOpenExternalUrl =
      typeof window.astrbotDesktop?.openExternalUrl === 'function'
        ? window.astrbotDesktop.openExternalUrl.bind(window.astrbotDesktop)
        : null;
    if (!bridgeOpenExternalUrl) return false;
    const href = url.toString();
    const isBridgeFailureResult = (result) =>
      !!result &&
      typeof result === 'object' &&
      Object.prototype.hasOwnProperty.call(result, 'ok') &&
      result.ok === false;

    try {
      const bridgeResult = bridgeOpenExternalUrl(href);
      if (bridgeResult && typeof bridgeResult.then === 'function') {
        Promise.resolve(bridgeResult)
          .then((result) => {
            if (isBridgeFailureResult(result)) {
              warnExternalUrlBridgeError('result', href, result.reason ?? result);
            }
          })
          .catch((error) => {
            warnExternalUrlBridgeError('async', href, error);
          });
        return true;
      }
      if (isBridgeFailureResult(bridgeResult)) {
        warnExternalUrlBridgeError('result', href, bridgeResult.reason ?? bridgeResult);
        return false;
      }
      return true;
    } catch (error) {
      warnExternalUrlBridgeError('sync', href, error);
      return false;
    }
  };

  const findAnchorFromEvent = (event) => {
    const target = event.target;
    if (target instanceof Element && typeof target.closest === 'function') {
      const direct = target.closest('a[href]');
      if (direct instanceof HTMLAnchorElement) return direct;
    }
    // Support anchors inside shadow DOM by walking the composed event path.
    if (typeof event.composedPath === 'function') {
      for (const node of event.composedPath()) {
        if (node instanceof HTMLAnchorElement && node.hasAttribute('href')) {
          return node;
        }
      }
    }
    return null;
  };
  const patchLocationHref = (locationObject) => {
    const descriptor =
      Object.getOwnPropertyDescriptor(locationObject, 'href') ||
      Object.getOwnPropertyDescriptor(window.Location?.prototype ?? {}, 'href');
    const nativeHrefGetter =
      descriptor && typeof descriptor.get === 'function'
        ? descriptor.get.bind(locationObject)
        : null;
    const nativeHrefSetter =
      descriptor && typeof descriptor.set === 'function'
        ? descriptor.set.bind(locationObject)
        : null;
    if (!nativeHrefSetter) return;

    safePatch('location.href', () => {
      Object.defineProperty(locationObject, 'href', {
        configurable: true,
        enumerable: descriptor?.enumerable ?? true,
        get() {
          if (nativeHrefGetter) {
            return nativeHrefGetter();
          }
          return locationObject.toString();
        },
        set(url) {
          if (openExternalUrl(url)) {
            return;
          }
          nativeHrefSetter(url);
        },
      });
    });
  };

  // Best-effort fake Window-like handle for callers that only check `closed` and `location.href`.
  const createWindowOpenHandle = (url) => {
    let closed = false;
    const locationProxy = {
      href: String(url ?? ''),
    };
    return {
      get closed() {
        return closed;
      },
      set closed(value) {
        closed = !!value;
      },
      close: () => {
        closed = true;
      },
      focus: () => {},
      location: locationProxy,
      opener: null,
    };
  };

  const installExternalAnchorInterceptor = () => {
    document.addEventListener(
      'click',
      (event) => {
        if (
          event.defaultPrevented ||
          event.button !== 0 ||
          event.metaKey ||
          event.ctrlKey ||
          event.shiftKey ||
          event.altKey
        ) {
          return;
        }

        const anchor = findAnchorFromEvent(event);
        if (!anchor) return;
        if (anchor.hasAttribute('download')) return;
        const rawHref = anchor.getAttribute('href') || anchor.href || '';
        if (!openExternalUrl(rawHref)) return;

        event.preventDefault();
      },
      true,
    );
  };

  const installWindowOpenBridge = () => {
    const nativeWindowOpen =
      typeof window.open === 'function' ? window.open.bind(window) : null;

    const bridgeWindowOpen = (url, target, features) => {
      if (target === '_self' || target === '_top' || target === '_parent') {
        if (nativeWindowOpen) {
          return nativeWindowOpen(url, target, features);
        }
        return null;
      }

      if (openExternalUrl(url)) {
        // Lightweight window-like handle for callers that only check basic fields.
        return createWindowOpenHandle(url);
      }

      if (nativeWindowOpen) {
        return nativeWindowOpen(url, target, features);
      }
      return null;
    };

    if (nativeWindowOpen) {
      safePatch('window.__astrbotNativeWindowOpen', () => {
        Object.defineProperty(window, '__astrbotNativeWindowOpen', {
          configurable: true,
          writable: false,
          enumerable: false,
          value: nativeWindowOpen,
        });
      });
    }

    safePatch('window.open', () => {
      window.open = bridgeWindowOpen;
    });
  };

  const installLocationNavigationBridge = () => {
    const locationObject = window.location;
    const nativeAssign =
      typeof locationObject.assign === 'function'
        ? locationObject.assign.bind(locationObject)
        : null;
    const nativeReplace =
      typeof locationObject.replace === 'function'
        ? locationObject.replace.bind(locationObject)
        : null;

    if (nativeAssign) {
      safePatch('location.assign', () => {
        locationObject.assign = (url) => {
          if (openExternalUrl(url)) {
            return;
          }
          nativeAssign(url);
        };
      });
    }

    if (nativeReplace) {
      safePatch('location.replace', () => {
        locationObject.replace = (url) => {
          if (openExternalUrl(url)) {
            return;
          }
          nativeReplace(url);
        };
      });
    }

    patchLocationHref(locationObject);
  };
  let navigationBridgesInstalled = false;
  const installNavigationBridges = () => {
    if (navigationBridgesInstalled) return;
    navigationBridgesInstalled = true;
    installWindowOpenBridge();
    installExternalAnchorInterceptor();
    installLocationNavigationBridge();
  };

  const RUNTIME_BRIDGE_DETAIL_MAX_LENGTH = 240;
  const RUNTIME_BRIDGE_DETAIL_MAX_ITEMS = 8;
  const RUNTIME_BRIDGE_TRUE_STRINGS = new Set(['1', 'true', 'yes', 'on']);
  const RUNTIME_BRIDGE_FALSE_STRINGS = new Set(['0', 'false', 'no', 'off']);
  const RUNTIME_BRIDGE_SENSITIVE_KEY_PATTERN =
    /(token|secret|password|passwd|authorization|cookie|api[_-]?key|access[_-]?key|refresh[_-]?token|credential)/i;

  const truncateRuntimeBridgeDetail = (value) => {
    if (typeof value !== 'string') {
      return value;
    }
    if (value.length <= RUNTIME_BRIDGE_DETAIL_MAX_LENGTH) {
      return value;
    }
    return `${value.slice(0, RUNTIME_BRIDGE_DETAIL_MAX_LENGTH)}...`;
  };

  const isSensitiveRuntimeBridgeKey = (key) =>
    typeof key === 'string' && RUNTIME_BRIDGE_SENSITIVE_KEY_PATTERN.test(key);

  const summarizeRuntimeBridgeValue = (value, depth = 0) => {
    if (
      value === null ||
      typeof value === 'string' ||
      typeof value === 'number' ||
      typeof value === 'boolean'
    ) {
      return value;
    }

    if (value instanceof Error) {
      return truncateRuntimeBridgeDetail(`${value.name}: ${value.message}`);
    }

    if (typeof value === 'undefined') {
      return '[undefined]';
    }

    if (typeof value === 'function') {
      return '[function]';
    }

    if (typeof value !== 'object') {
      return `[${typeof value}]`;
    }

    if (depth >= 1) {
      return Array.isArray(value) ? `[array:${value.length}]` : '[object]';
    }

    if (Array.isArray(value)) {
      const sample = value
        .slice(0, RUNTIME_BRIDGE_DETAIL_MAX_ITEMS)
        .map((item) => summarizeRuntimeBridgeValue(item, depth + 1));
      if (value.length > sample.length) {
        sample.push(`[+${value.length - sample.length} items]`);
      }
      return sample;
    }

    const keys = Object.keys(value);
    const sampledKeys = keys.slice(0, RUNTIME_BRIDGE_DETAIL_MAX_ITEMS);
    const sampled = {};
    for (const key of sampledKeys) {
      if (isSensitiveRuntimeBridgeKey(key)) {
        sampled[key] = '[redacted]';
        continue;
      }
      sampled[key] = summarizeRuntimeBridgeValue(value[key], depth + 1);
    }
    if (keys.length > sampledKeys.length) {
      sampled.__omittedKeys = keys.length - sampledKeys.length;
    }
    return sampled;
  };

  const stringifyRuntimeBridgeDetail = (value) => {
    if (!value || typeof value !== 'object') {
      return `type=${Object.prototype.toString.call(value)}`;
    }

    try {
      return truncateRuntimeBridgeDetail(
        JSON.stringify(summarizeRuntimeBridgeValue(value)),
      );
    } catch {
      return `type=${Object.prototype.toString.call(value)}`;
    }
  };

  const sanitizeRuntimeBridgeDetail = (detail) => {
    if (detail instanceof Error) {
      return truncateRuntimeBridgeDetail(`${detail.name}: ${detail.message}`);
    }

    if (typeof detail === 'string') {
      return truncateRuntimeBridgeDetail(detail);
    }

    if (typeof detail === 'number' || typeof detail === 'boolean') {
      return String(detail);
    }

    if (detail && typeof detail === 'object') {
      const hasReason = typeof detail.reason === 'string' && detail.reason;
      const hasOk = typeof detail.ok === 'boolean';
      if (hasReason || hasOk) {
        const summary = [];
        if (hasOk) {
          summary.push(`ok=${detail.ok}`);
        }
        if (hasReason) {
          summary.push(`reason=${detail.reason}`);
        }
        return truncateRuntimeBridgeDetail(summary.join(' '));
      }
      return stringifyRuntimeBridgeDetail(detail);
    }

    return String(detail);
  };

  const logRuntimeBridgeFallback = (command, fallbackValue, detail) => {
    if (typeof console !== 'undefined' && typeof console.warn === 'function') {
      const sanitizedDetail = sanitizeRuntimeBridgeDetail(detail);
      console.warn(
        `[astrbotDesktop] ${command} fallback to ${fallbackValue}`,
        sanitizedDetail,
      );
    }
  };

  const getStrictBooleanFallback = (command, fallbackValue) => {
    if (typeof fallbackValue === 'boolean') {
      return fallbackValue;
    }
    if (typeof fallbackValue === 'undefined') {
      return false;
    }
    if (fallbackValue === null) {
      return false;
    }
    if (typeof fallbackValue === 'number') {
      if (fallbackValue === 1) {
        return true;
      }
      if (fallbackValue === 0) {
        return false;
      }
      logRuntimeBridgeFallback(
        command,
        false,
        `invalid numeric fallback (${fallbackValue}), force false`,
      );
      return false;
    }
    if (typeof fallbackValue === 'string') {
      const normalized = fallbackValue.trim().toLowerCase();
      if (RUNTIME_BRIDGE_TRUE_STRINGS.has(normalized)) {
        return true;
      }
      if (RUNTIME_BRIDGE_FALSE_STRINGS.has(normalized)) {
        return false;
      }
      logRuntimeBridgeFallback(
        command,
        false,
        `invalid string fallback (${truncateRuntimeBridgeDetail(fallbackValue)}), force false`,
      );
      return false;
    }

    logRuntimeBridgeFallback(
      command,
      false,
      `invalid fallback type (${typeof fallbackValue}), force false`,
    );
    return false;
  };

  const isRuntimeBridgeEnabled = async (command, fallbackValue) => {
    const normalizedFallback = getStrictBooleanFallback(command, fallbackValue);

    try {
      const result = await invokeBridge(command);
      if (typeof result === 'boolean') {
        return result;
      }
      logRuntimeBridgeFallback(
        command,
        normalizedFallback,
        `non-boolean result: ${String(result)}`,
      );
    } catch (error) {
      logRuntimeBridgeFallback(command, normalizedFallback, error);
    }

    return normalizedFallback;
  };

  const patchLocalStorageTokenSync = () => {
    if (window.__astrbotDesktopTokenSyncPatched) return;
    try {
      const storage = window.localStorage;
      if (!storage) return;
      window.__astrbotDesktopTokenSyncPatched = true;

      const rawSetItem = storage.setItem?.bind(storage);
      const rawRemoveItem = storage.removeItem?.bind(storage);
      const rawClear = storage.clear?.bind(storage);

      if (typeof rawSetItem === 'function') {
        storage.setItem = (key, value) => {
          rawSetItem(key, value);
          if (key === 'token') {
            void syncAuthToken(value);
          }
        };
      }
      if (typeof rawRemoveItem === 'function') {
        storage.removeItem = (key) => {
          rawRemoveItem(key);
          if (key === 'token') {
            void syncAuthToken(null);
          }
        };
      }
      if (typeof rawClear === 'function') {
        storage.clear = () => {
          rawClear();
          void syncAuthToken(null);
        };
      }
    } catch {}
  };

  window.astrbotDesktop = {
    __tauriBridge: true,
    isDesktop: true,
    isDesktopRuntime: () =>
      isRuntimeBridgeEnabled(BRIDGE_COMMANDS.IS_DESKTOP_RUNTIME, true),
    getBackendState: () => invokeBridge(BRIDGE_COMMANDS.GET_BACKEND_STATE),
    restartBackend: async (authToken = null) => {
      const normalizedToken =
        typeof authToken === 'string' && authToken ? authToken : getStoredAuthToken();
      await syncAuthToken(normalizedToken);
      return invokeBridge(BRIDGE_COMMANDS.RESTART_BACKEND, {
        authToken: normalizedToken
      });
    },
    stopBackend: () => invokeBridge(BRIDGE_COMMANDS.STOP_BACKEND),
    openExternalUrl: (url) => {
      const rawUrl = typeof url === 'string' ? url : String(url ?? '');
      if (!rawUrl.trim()) {
        return Promise.resolve({
          ok: true,
          reason: null,
        });
      }
      return invokeBridge(BRIDGE_COMMANDS.OPEN_EXTERNAL_URL, {
        url: rawUrl,
      });
    },
    onTrayRestartBackend,
  };

  installNavigationBridges();
  void listenToTrayRestartBackendEvent();
  patchLocalStorageTokenSync();
  void syncAuthToken();
})();
