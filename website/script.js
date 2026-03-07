(() => {
  "use strict"

  const translations = {
    en: {
      lang: "en",
      title: "CrateBay — Local AI managed sandboxes in one GUI · Coming Soon",
      description:
        "CrateBay — desktop GUI for local AI managed sandboxes, local models, MCP, and local AI infrastructure.",
      keywords: "cratebay, ai managed sandbox, local models, mcp, agents, desktop gui, coming soon",
      brand: "CrateBay",
      comingSoon: "Coming Soon",
      heroTitle: "Local AI managed sandboxes, in one GUI.",
      heroLead:
        "Managed sandboxes. Local models. MCP. Containers, VMs, and Kubernetes — all in one desktop app.",
      heroSub:
        "Built for fast local AI workflows without the mess of scripts, terminals, and scattered tools.",
      githubCta: "GitHub",
      summary1Label: "AI Sandboxes",
      summary1Title: "Run locally",
      summary1Body:
        "Create, start, stop, and manage built-in local AI sandboxes visually.",
      summary2Label: "Local Models",
      summary2Title: "One-click local models",
      summary2Body:
        "Pull, manage, and run local models from the same desktop surface.",
      summary3Label: "MCP + Infra",
      summary3Title: "One place for everything",
      summary3Body:
        "Keep MCP, models, containers, VMs, Kubernetes, and executable skills in one control plane.",
      sectionKicker: "Why It Hits",
      sectionTitle: "Local AI is hot. The workflow is still broken.",
      sectionBody:
        "Bring up a model, start a managed sandbox, connect tools, and iterate — all inside one desktop GUI.",
      card1Title: "Sandbox-first GUI",
      card1Body:
        "Managed sandboxes are a first-class desktop workflow.",
      card2Title: "One-click local models",
      card2Body:
        "Make local model setup fast, visual, and daily-use ready.",
      card3Title: "MCP built into the desktop",
      card3Body:
        "Bring MCP servers, Codex / Claude bridges, assistant flows, and quick skills into the same desktop environment.",
      card4Title: "Your local AI stack, in one app",
      card4Body:
        "Keep models, managed sandboxes, and infra in one app.",
      statusKicker: "Status",
      statusTitle: "Coming soon",
      statusBody:
        "For builders who want a better local AI workflow.",
      statusNote:
        "The preview now ships with automated UI, Docker runtime, and AI runtime validation across the core desktop flows.",
      footer: "CrateBay · <span data-year></span>",
    },
    zh: {
      lang: "zh-CN",
      title: "CrateBay — 本地 AI 托管沙箱，一体化桌面 GUI · 即将推出",
      description:
        "CrateBay —— 面向本地 AI 托管沙箱、本地模型、MCP 与本地 AI 基础设施的一体化桌面 GUI。",
      keywords: "cratebay, ai managed sandbox, 本地模型, mcp, agents, desktop gui, 即将推出",
      brand: "CrateBay",
      comingSoon: "即将推出",
      heroTitle: "本地 AI 托管沙箱，一体化 GUI。",
      heroLead:
        "托管沙箱、本地模型、MCP、容器、VM、Kubernetes —— 全都放进一个桌面应用里。",
      heroSub:
        "为更快的本地 AI 工作流而生：少一点终端和脚本，多一点速度与掌控感。",
      githubCta: "GitHub",
      summary1Label: "AI Sandboxes",
      summary1Title: "本地运行，可视化管理",
      summary1Body:
        "用桌面 GUI 创建、启动、停止并管理由 CrateBay 托管的本地 AI 沙箱。",
      summary2Label: "Local Models",
      summary2Title: "一键本地模型",
      summary2Body:
        "在同一个桌面界面里拉取、管理和运行本地模型。",
      summary3Label: "MCP + Infra",
      summary3Title: "一个界面管理全栈",
      summary3Body:
        "把 MCP、模型、容器、VM、Kubernetes 与可执行 skills 统一放进一个桌面控制面。",
      sectionKicker: "为什么它有吸引力",
      sectionTitle: "本地 AI 很火，但真正顺手的工作流还不多。",
      sectionBody:
        "拉起模型、启动托管沙箱、连接工具、快速迭代，都在一个桌面 GUI 里完成。",
      card1Title: "Sandbox-first GUI",
      card1Body:
        "由 CrateBay 托管的 AI 沙箱，不该只是 CLI 背后的专家功能。",
      card2Title: "一键本地模型",
      card2Body:
        "让本地模型部署与管理更快、更直观。",
      card3Title: "桌面内建 MCP",
      card3Body:
        "把 MCP server、Codex / Claude 桥接、assistant 工作流与快捷 skills 放进同一个桌面环境里。",
      card4Title: "你的本地 AI 栈，一个应用搞定",
      card4Body:
        "把模型、托管沙箱和基础设施放进一个应用。",
      statusKicker: "状态",
      statusTitle: "即将推出",
      statusBody:
        "如果你想要更好的本地 AI 工作流，就关注 CrateBay。",
      statusNote:
        "当前预览版本已补齐核心桌面流程的自动化 UI、Docker runtime 与 AI runtime 验证。",
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
