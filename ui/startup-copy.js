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

const STARTUP_PANEL_COMMANDS = {
  GET_SNAPSHOT: 'desktop_bridge_get_startup_panel_snapshot',
};

const STARTUP_PANEL_POLL_INTERVAL_MS = 1500;

window.astrbot = window.astrbot || {};
window.astrbot.startupShell = deepFreeze({
  STARTUP_MODES,
  STARTUP_PANEL_COMMANDS,
  STARTUP_PANEL_POLL_INTERVAL_MS,
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
  STARTUP_DIAGNOSTICS_COPY: {
    en: {
      summaryLabel: 'Startup',
      showDetails: 'Details',
      hideDetails: 'Hide',
      stageListLabel: 'Startup stages',
      stageSummaries: {
        resolveLaunchPlan: 'Resolving launch plan',
        spawnBackend: 'Spawning backend',
        tcpReachable: 'TCP ready, waiting for HTTP',
        httpReady: 'Backend ready',
        failed: 'Startup failed',
      },
      desktopLogLabel: 'Desktop',
      backendLogLabel: 'Backend',
      emptyLogLabel: 'No recent lines',
      stageLabels: {
        plan: 'Plan',
        spawn: 'Spawn',
        tcp: 'TCP',
        http: 'HTTP',
      },
    },
    zh: {
      summaryLabel: '启动',
      showDetails: '详情',
      hideDetails: '收起',
      stageListLabel: '启动阶段',
      stageSummaries: {
        resolveLaunchPlan: '正在解析启动计划',
        spawnBackend: '正在启动后端',
        tcpReachable: 'TCP 已就绪，正在等待 HTTP',
        httpReady: '后端已就绪',
        failed: '启动失败',
      },
      desktopLogLabel: '桌面端',
      backendLogLabel: '后端',
      emptyLogLabel: '暂无最近日志',
      stageLabels: {
        plan: '计划',
        spawn: '拉起',
        tcp: 'TCP',
        http: 'HTTP',
      },
    },
  },
});
