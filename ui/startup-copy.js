const deepFreeze = (obj) => {
  for (const key of Object.getOwnPropertyNames(obj)) {
    const value = obj[key];
    if (value && typeof value === 'object' && !Object.isFrozen(value)) {
      deepFreeze(value);
    }
  }

  return Object.freeze(obj);
};

const STARTUP_MODES = {
  LOADING: 'loading',
  PANEL_UPDATE: 'panel-update',
};

window.astrbot = window.astrbot || {};
window.astrbot.startupShell = deepFreeze({
  STARTUP_MODES,
  STARTUP_COPY: {
    en: {
      [STARTUP_MODES.LOADING]: {
        title: 'AstrBot Desktop',
        desc: 'Preparing the runtime, please wait.',
        status: 'Starting core services...',
      },
      [STARTUP_MODES.PANEL_UPDATE]: {
        title: 'AstrBot Desktop',
        desc: 'A new panel version is available and is being downloaded and applied.',
        status: 'Syncing panel assets...',
      },
    },
    zh: {
      [STARTUP_MODES.LOADING]: {
        title: 'AstrBot Desktop',
        desc: '正在准备运行环境，请稍候。',
        status: '正在启动核心服务...',
      },
      [STARTUP_MODES.PANEL_UPDATE]: {
        title: 'AstrBot Desktop',
        desc: '检测到新面板版本，正在下载并应用。',
        status: '正在同步面板资源...',
      },
    },
  },
});
