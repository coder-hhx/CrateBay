(() => {
  "use strict"

  const translations = {
    en: {
      lang: "en",
      title: "CrateBay — Local AI sandboxes in one GUI · Coming Soon",
      description:
        "CrateBay — desktop GUI for local AI sandboxes, local models, MCP, and local AI infrastructure.",
      keywords: "cratebay, ai sandbox, local models, mcp, agents, desktop gui, coming soon",
      brand: "CrateBay",
      comingSoon: "Coming Soon",
      heroTitle: "Local AI sandboxes, in one GUI.",
      heroLead:
        "AI sandboxes. Local models. MCP. Containers, VMs, and Kubernetes — all in one desktop app.",
      heroSub:
        "Built for fast local AI workflows without the mess of scripts, terminals, and scattered tools.",
      githubCta: "GitHub",
      summary1Label: "AI Sandboxes",
      summary1Title: "Run locally",
      summary1Body:
        "Create, start, stop, and manage local AI sandboxes visually.",
      summary2Label: "Local Models",
      summary2Title: "One-click local models",
      summary2Body:
        "Pull, manage, and run local models from the same desktop surface.",
      summary3Label: "MCP + Infra",
      summary3Title: "One place for everything",
      summary3Body:
        "Keep MCP, models, containers, VMs, and Kubernetes in one control plane.",
      sectionKicker: "Why It Hits",
      sectionTitle: "Local AI is hot. The workflow is still broken.",
      sectionBody:
        "Bring up a model, start a sandbox, connect tools, and iterate — all inside one desktop GUI.",
      card1Title: "Sandbox-first GUI",
      card1Body:
        "AI sandboxes are a first-class desktop workflow.",
      card2Title: "One-click local models",
      card2Body:
        "Make local model setup fast, visual, and daily-use ready.",
      card3Title: "MCP built into the desktop",
      card3Body:
        "Bring MCP servers and assistant flows into the same desktop environment.",
      card4Title: "Your local AI stack, in one app",
      card4Body:
        "Keep models, sandboxes, and infra in one app.",
      statusKicker: "Status",
      statusTitle: "Coming soon",
      statusBody:
        "For builders who want a better local AI workflow.",
      footer: "CrateBay · <span data-year></span>",
    },
    zh: {
      lang: "zh-CN",
      title: "CrateBay — 本地 AI Sandbox，一体化桌面 GUI · 即将推出",
      description:
        "CrateBay —— 面向本地 AI sandbox、本地模型、MCP 与本地 AI 基础设施的一体化桌面 GUI。",
      keywords: "cratebay, ai sandbox, 本地模型, mcp, agents, desktop gui, 即将推出",
      brand: "CrateBay",
      comingSoon: "即将推出",
      heroTitle: "本地 AI Sandbox，一体化 GUI。",
      heroLead:
        "AI sandbox、本地模型、MCP、容器、VM、Kubernetes —— 全都放进一个桌面应用里。",
      heroSub:
        "为更快的本地 AI 工作流而生：少一点终端和脚本，多一点速度与掌控感。",
      githubCta: "GitHub",
      summary1Label: "AI Sandboxes",
      summary1Title: "本地运行，可视化管理",
      summary1Body:
        "用桌面 GUI 创建、启动、停止并管理本地 AI sandbox。",
      summary2Label: "Local Models",
      summary2Title: "一键本地模型",
      summary2Body:
        "在同一个桌面界面里拉取、管理和运行本地模型。",
      summary3Label: "MCP + Infra",
      summary3Title: "一个界面管理全栈",
      summary3Body:
        "把 MCP、模型、容器、VM、Kubernetes 统一放进一个桌面控制面。",
      sectionKicker: "为什么它有吸引力",
      sectionTitle: "本地 AI 很火，但真正顺手的工作流还不多。",
      sectionBody:
        "拉起模型、启动 sandbox、连接工具、快速迭代，都在一个桌面 GUI 里完成。",
      card1Title: "Sandbox-first GUI",
      card1Body:
        "本地 AI sandbox，不该只是 CLI 背后的专家功能。",
      card2Title: "一键本地模型",
      card2Body:
        "让本地模型部署与管理更快、更直观。",
      card3Title: "桌面内建 MCP",
      card3Body:
        "把 MCP server 和 assistant 工作流放进同一个桌面环境里。",
      card4Title: "你的本地 AI 栈，一个应用搞定",
      card4Body:
        "把模型、sandbox 和基础设施放进一个应用。",
      statusKicker: "状态",
      statusTitle: "即将推出",
      statusBody:
        "如果你想要更好的本地 AI 工作流，就关注 CrateBay。",
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
