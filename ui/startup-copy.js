(() => {
  const STARTUP_MODES = Object.freeze({
    LOADING: 'loading',
    PANEL_UPDATE: 'panel-update',
  });

  const STARTUP_COPY = Object.freeze({
    en: Object.freeze({
      [STARTUP_MODES.LOADING]: Object.freeze({
        title: 'AstrBot Desktop',
        desc: 'Preparing the runtime, please wait.',
        status: 'Starting core services...',
      }),
      [STARTUP_MODES.PANEL_UPDATE]: Object.freeze({
        title: 'AstrBot Desktop',
        desc: 'A new panel version is available and is being downloaded and applied.',
        status: 'Syncing panel assets...',
      }),
    }),
    zh: Object.freeze({
      [STARTUP_MODES.LOADING]: Object.freeze({
        title: 'AstrBot Desktop',
        desc: '正在准备运行环境，请稍候。',
        status: '正在启动核心服务...',
      }),
      [STARTUP_MODES.PANEL_UPDATE]: Object.freeze({
        title: 'AstrBot Desktop',
        desc: '检测到新面板版本，正在下载并应用。',
        status: '正在同步面板资源...',
      }),
    }),
  });

  window.__astrbotStartupShell = Object.freeze({
    STARTUP_MODES,
    STARTUP_COPY,
  });
})();
