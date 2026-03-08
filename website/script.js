(() => {
  "use strict"

  const translations = {
    en: {
      lang: "en",
      title: "CrateBay — Local AI control plane in one GUI · Coming Soon",
      description:
        "CrateBay — desktop GUI for local AI sandboxes, local models, MCP servers, and provider / CLI bridges.",
      keywords: "cratebay, local ai, ai sandbox, local models, mcp, provider bridge, codex, claude, desktop gui",
      brand: "CrateBay",
      comingSoon: "Coming Soon",
      heroTitle: "Your local AI stack, in one GUI.",
      heroLead:
        "Managed sandboxes. Local models. MCP. Provider and CLI bridges — all in one desktop app.",
      heroSub:
        "Built for fast local AI workflows, with Docker-backed runtimes underneath and heavyweight infra surfaces deferred past v1.",
      githubCta: "GitHub",
      summary1Label: "AI Sandboxes",
      summary1Title: "Run locally",
      summary1Body:
        "Create, start, stop, and manage built-in local AI sandboxes visually.",
      summary2Label: "Local Models",
      summary2Title: "One-click local models",
      summary2Body:
        "Pull, manage, and run local models from the same desktop surface.",
      summary3Label: "MCP + Bridges",
      summary3Title: "Connect tools fast",
      summary3Body:
        "Keep MCP servers, provider profiles, Codex / Claude bridges, and local runtimes in one control plane.",
      sectionKicker: "Why It Hits",
      sectionTitle: "Local AI is hot. The workflow is still broken.",
      sectionBody:
        "Bring up a model, start a managed sandbox, connect MCP and bridges, and iterate — all inside one desktop GUI.",
      card1Title: "Sandbox-first GUI",
      card1Body:
        "Managed sandboxes are a first-class desktop workflow.",
      card2Title: "One-click local models",
      card2Body:
        "Make local model setup fast, visual, and daily-use ready.",
      card3Title: "MCP and bridges built in",
      card3Body:
        "Bring MCP servers, Codex / Claude bridges, and lightweight assistant flows into the same desktop environment.",
      card4Title: "Focus on the AI workflow",
      card4Body:
        "Use containers as the engine under the hood while v1 stays focused on sandboxes, models, and tool connections.",
      statusKicker: "Status",
      statusTitle: "Coming soon",
      statusBody:
        "For builders who want a better local AI workflow.",
      statusNote:
        "The preview now ships with automated UI, Docker runtime, AI runtime, Linux desktop runtime validation, and local GPU telemetry for model and sandbox runtimes; the desktop dashboard and navigation now prioritize sandboxes, models, MCP, settings-driven bridges, and controlled provider canaries, while VMs and Kubernetes remain post-v1 tracks.",
      footer: "CrateBay · <span data-year></span>",
    },
    zh: {
      lang: "zh-CN",
      title: "CrateBay — 本地 AI 控制台，一体化桌面 GUI · 即将推出",
      description:
        "CrateBay —— 面向本地 AI 沙箱、本地模型、MCP Server 与 provider / CLI 桥接的一体化桌面 GUI。",
      keywords: "cratebay, local ai, ai sandbox, 本地模型, mcp, provider bridge, codex, claude, desktop gui",
      brand: "CrateBay",
      comingSoon: "即将推出",
      heroTitle: "你的本地 AI 栈，一个 GUI 搞定。",
      heroLead:
        "托管沙箱、本地模型、MCP、provider 与 CLI 桥接 —— 全都放进一个桌面应用里。",
      heroSub:
        "为更快的本地 AI 工作流而生：底层由 Docker 运行时支撑，重量级基础设施能力延后到 v1 之后。",
      githubCta: "GitHub",
      summary1Label: "AI Sandboxes",
      summary1Title: "本地运行，可视化管理",
      summary1Body:
        "用桌面 GUI 创建、启动、停止并管理由 CrateBay 托管的本地 AI 沙箱。",
      summary2Label: "Local Models",
      summary2Title: "一键本地模型",
      summary2Body:
        "在同一个桌面界面里拉取、管理和运行本地模型。",
      summary3Label: "MCP + Bridges",
      summary3Title: "快速连通工具链",
      summary3Body:
        "把 MCP Server、provider 配置、Codex / Claude 桥接与本地运行时统一放进一个桌面控制面。",
      sectionKicker: "为什么它有吸引力",
      sectionTitle: "本地 AI 很火，但真正顺手的工作流还不多。",
      sectionBody:
        "拉起模型、启动托管沙箱、连接 MCP 与桥接工具、快速迭代，都在一个桌面 GUI 里完成。",
      card1Title: "Sandbox-first GUI",
      card1Body:
        "由 CrateBay 托管的 AI 沙箱，不该只是 CLI 背后的专家功能。",
      card2Title: "一键本地模型",
      card2Body:
        "让本地模型部署与管理更快、更直观。",
      card3Title: "桌面内建 MCP 与桥接",
      card3Body:
        "把 MCP Server、Codex / Claude 桥接与轻量 assistant 工作流放进同一个桌面环境里。",
      card4Title: "聚焦本地 AI 工作流",
      card4Body:
        "让容器在底层提供运行时支撑，同时让 v1 聚焦沙箱、模型与工具连接。",
      statusKicker: "状态",
      statusTitle: "即将推出",
      statusBody:
        "如果你想要更好的本地 AI 工作流，就关注 CrateBay。",
      statusNote:
        "当前预览版本已补齐自动化 UI、Docker runtime、AI runtime、Linux 桌面运行时验证，以及面向本地模型与沙箱运行时的 GPU 遥测；桌面 dashboard 与导航现已优先聚焦沙箱、模型、MCP、由设置驱动的 bridges，以及受控 provider canary，VM 与 Kubernetes 则放到 v1 之后。",
      footer: "CrateBay · <span data-year></span>",
    },
  }

  const storageKey = "cratebay-site-lang"
  const titleNode = document.querySelector("title")
  const descriptionMeta = document.querySelector('meta[name="description"]')
  const keywordsMeta = document.querySelector('meta[name="keywords"]')
  const year = String(new Date().getFullYear())

  function renderFooter() {
    document.querySelectorAll("[data-year]").forEach((node) => {
      node.textContent = year
    })
  }

  function setLanguage(lang) {
    const next = translations[lang] ? lang : "en"
    const dict = translations[next]
    document.documentElement.lang = dict.lang
    if (titleNode) titleNode.textContent = dict.title
    if (descriptionMeta) descriptionMeta.setAttribute("content", dict.description)
    if (keywordsMeta) keywordsMeta.setAttribute("content", dict.keywords)

    document.querySelectorAll("[data-i18n]").forEach((node) => {
      const key = node.getAttribute("data-i18n")
      if (!key || !(key in dict)) return
      if (key === "footer") {
        node.innerHTML = dict[key]
      } else {
        node.textContent = dict[key]
      }
    })

    document.querySelectorAll(".lang-btn").forEach((button) => {
      const active = button.getAttribute("data-lang") === next
      button.setAttribute("aria-pressed", active ? "true" : "false")
    })

    renderFooter()
    localStorage.setItem(storageKey, next)
  }

  const saved = localStorage.getItem(storageKey)
  const initial = saved || (navigator.language && navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en")
  setLanguage(initial)

  document.querySelectorAll(".lang-btn").forEach((button) => {
    button.addEventListener("click", () => {
      const lang = button.getAttribute("data-lang") || "en"
      setLanguage(lang)
    })
  })
})()
