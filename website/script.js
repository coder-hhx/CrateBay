(() => {
  "use strict"

  const translations = {
    en: {
      lang: "en",
      title: "CrateBay — The GUI for local AI sandboxes · Coming Soon",
      description:
        "CrateBay — the desktop GUI for local AI sandboxes, one-click local model deployment, MCP servers, and modern local AI infrastructure workflows.",
      keywords: "cratebay, ai sandbox, local models, mcp, agents, desktop gui, coming soon",
      brand: "CrateBay",
      comingSoon: "Coming Soon",
      heroTitle: "The GUI for local AI sandboxes.",
      heroLead:
        "Spin up AI sandboxes. Deploy local models in one click. Manage MCP servers, containers, VMs, and Kubernetes — all from one native desktop app.",
      heroSub:
        "Local AI is moving fast. CrateBay is built for AI builders who want the speed of modern agent workflows without the chaos of terminals, scripts, and scattered dashboards.",
      githubCta: "GitHub",
      summary1Label: "AI Sandboxes",
      summary1Title: "Create and control locally",
      summary1Body:
        "Run local AI sandboxes from a real desktop GUI instead of stitching together shell commands by hand.",
      summary2Label: "Local Models",
      summary2Title: "One-click deployment flow",
      summary2Body:
        "Pull, manage, and run local models from the same product surface where the rest of your stack already lives.",
      summary3Label: "MCP + Infra",
      summary3Title: "One place for the whole stack",
      summary3Body:
        "Keep MCP servers, containers, Linux VMs, Kubernetes, and assistant-driven workflows inside one desktop control plane.",
      sectionKicker: "Why It Hits",
      sectionTitle: "Local AI is exploding. The workflow is still messy.",
      sectionBody:
        "CrateBay is designed around the workflow people actually want right now: bring up a model, start a sandbox, connect tools, and iterate fast — without leaving your desktop GUI.",
      card1Title: "Sandbox-first desktop UX",
      card1Body:
        "Local AI sandboxes are treated as a first-class workflow, not a hidden expert-only feature behind CLI tooling.",
      card2Title: "One-click local models",
      card2Body:
        "Make local model deployment and management feel fast, visual, and ready for daily AI work.",
      card3Title: "MCP built into the desktop",
      card3Body:
        "Bring tool servers and assistant flows into the same environment where your local infrastructure already runs.",
      card4Title: "Your local AI stack, in one app",
      card4Body:
        "From models and sandboxes to containers and VMs, CrateBay is built to become the control surface for local AI development.",
      statusKicker: "Status",
      statusTitle: "Coming soon for AI builders",
      statusBody:
        "If you care about local AI sandboxes, local models, MCP workflows, and desktop-first agent tooling, keep an eye on CrateBay.",
      footer: "CrateBay · <span data-year></span>",
    },
    zh: {
      lang: "zh-CN",
      title: "CrateBay — 本地 AI Sandbox 的桌面 GUI · 即将推出",
      description:
        "CrateBay —— 面向本地 AI sandbox、一键本地模型部署、MCP server 与现代本地 AI 基础设施工作流的桌面 GUI。",
      keywords: "cratebay, ai sandbox, 本地模型, mcp, agents, desktop gui, 即将推出",
      brand: "CrateBay",
      comingSoon: "即将推出",
      heroTitle: "本地 AI Sandbox 的桌面 GUI。",
      heroLead:
        "本地 AI sandbox，一键部署本地模型。MCP server、容器、VM、Kubernetes —— 全都放进一个原生桌面应用里。",
      heroSub:
        "本地 AI 发展很快。CrateBay 面向这波 AI builder，想解决的就是：少一点终端、脚本和分散面板，多一点速度、可视化和掌控感。",
      githubCta: "GitHub",
      summary1Label: "AI Sandboxes",
      summary1Title: "本地创建，可视化管理",
      summary1Body:
        "用真正的桌面 GUI 管理本地 AI sandbox，不再手搓一堆 shell 命令和零散脚本。",
      summary2Label: "Local Models",
      summary2Title: "一键本地模型流程",
      summary2Body:
        "在同一个产品界面里拉取、管理和运行本地模型，不用在多套工具之间来回切换。",
      summary3Label: "MCP + Infra",
      summary3Title: "整套本地栈放进一个界面",
      summary3Body:
        "把 MCP server、容器、Linux VM、Kubernetes 和 assistant 工作流统一放进一个桌面控制面。",
      sectionKicker: "为什么值得关注",
      sectionTitle: "本地 AI 很火，但工作流依然很乱。",
      sectionBody:
        "CrateBay 围绕现在最热门的本地 AI 工作流来设计：拉起模型、启动 sandbox、连上工具、快速迭代，而且整个过程都不必离开桌面 GUI。",
      card1Title: "Sandbox-first 桌面体验",
      card1Body:
        "本地 AI sandbox 是一等公民，不再只是藏在 CLI 后面的专家功能。",
      card2Title: "一键本地模型",
      card2Body:
        "让本地模型部署和管理变得更快、更直观、更适合日常 AI 开发。",
      card3Title: "桌面内建 MCP",
      card3Body:
        "把工具服务和 assistant 工作流直接放进你本地基础设施已经运行的那个桌面环境里。",
      card4Title: "你的本地 AI 栈，一个应用搞定",
      card4Body:
        "从模型和 sandbox，到容器和 VM，CrateBay 想成为本地 AI 开发的一体化控制面。",
      statusKicker: "状态",
      statusTitle: "面向 AI builder，即将推出",
      statusBody:
        "如果你关心本地 AI sandbox、本地模型、MCP 工作流和桌面优先的 agent tooling，那就关注 CrateBay。",
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
