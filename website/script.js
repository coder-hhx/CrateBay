(() => {
  "use strict"

  const translations = {
    en: {
      lang: "en",
      title: "CrateBay · Container and Image Management",
      description: "CrateBay — open-source desktop container and image management with a built-in runtime.",
      keywords: "cratebay, containers, images, docker, container runtime, desktop app, tauri, orbstack alternative",
      brand: "CrateBay",
      comingSoon: "v0.9 Alpha",
      heroTitle: "CrateBay",
      heroLead: "Container and image management for your local machine.",
      heroSub:
        "Browse images, pull from registries, package container filesystems, manage pods, and run one-shot container tasks through the CLI and built-in runtime.",
      githubCta: "GitHub",
      summary1Label: "Images",
      summary1Title: "Search, pull, inspect",
      summary1Body:
        "Manage local images, search registries, follow pull progress, tag images, and remove stale layers.",
      summary2Label: "Runtime",
      summary2Title: "Built-in runtime",
      summary2Body:
        "Start a local VM-backed container engine on macOS, Windows, and Linux without Docker Desktop.",
      summary3Label: "Containers",
      summary3Title: "Lifecycle tools",
      summary3Body:
        "Create, start, stop, inspect, exec, and view logs from a focused desktop interface.",
      summary4Label: "CLI",
      summary4Title: "Scriptable runs",
      summary4Body:
        "Use CLI + runtime as the minimum unit for controlled one-shot container execution.",
      sectionKicker: "Why CrateBay",
      sectionTitle: "A lean local alternative to Docker Desktop and OrbStack.",
      sectionBody:
        "CrateBay keeps the daily container workflow fast: runtime control, image pulls and packaging, pod grouping, container details, logs, and terminal access.",
      card1Title: "Image Management",
      card1Body:
        "Search Docker Hub, configure mirrors, pull images, tag, export, import, package containers, and clean up local storage.",
      card2Title: "Built-in Runtime",
      card2Body:
        "The self-managed runtime is the default path; external Docker endpoints are explicit compatibility overrides.",
      card3Title: "Pods and Bundles",
      card3Body:
        "Group related containers with managed Docker networks and preload bundled development images when needed.",
      card4Title: "Cross-Platform",
      card4Body:
        "macOS (Virtualization.framework), Linux (KVM), Windows (WSL2). Built with Tauri v2 for native performance.",
      statusKicker: "Status",
      statusTitle: "v0.9 \u2014 Images, Pods, Runtime",
      statusBody:
        "Built-in runtime, bundled images, image pull/pack/export/import, pod grouping, one-shot CLI runs, and container lifecycle are the current focus.",
      footer: "CrateBay \u00b7 <span data-year></span>",
    },
    zh: {
      lang: "zh-CN",
      title: "CrateBay \u00b7 \u5bb9\u5668\u548c\u955c\u50cf\u7ba1\u7406",
      description: "CrateBay \u2014\u2014 \u5f00\u6e90\u684c\u9762\u5bb9\u5668\u548c\u955c\u50cf\u7ba1\u7406\u5de5\u5177\uff0c\u5185\u7f6e\u8fd0\u884c\u65f6\u3002",
      keywords: "cratebay, \u5bb9\u5668, \u955c\u50cf, docker, \u8fd0\u884c\u65f6, \u684c\u9762\u5e94\u7528, tauri, orbstack",
      brand: "CrateBay",
      comingSoon: "v0.9 Alpha",
      heroTitle: "CrateBay",
      heroLead: "\u5bb9\u5668\u548c\u955c\u50cf\u7ba1\u7406\uff0c\u5c31\u5728\u4f60\u7684\u672c\u5730\u673a\u5668\u4e0a\u3002",
      heroSub:
        "浏览镜像、从仓库拉取、打包容器文件系统、管理 Pod，并通过 CLI + 内置运行时执行一次性容器任务。",
      githubCta: "GitHub",
      summary1Label: "\u955c\u50cf",
      summary1Title: "\u641c\u7d22\u3001\u62c9\u53d6\u3001\u67e5\u770b",
      summary1Body:
        "\u7edf\u4e00\u7ba1\u7406\u672c\u5730\u955c\u50cf\uff0c\u641c\u7d22\u4ed3\u5e93\u3001\u8ddf\u8e2a\u62c9\u53d6\u8fdb\u5ea6\u3001\u6dfb\u52a0\u6807\u7b7e\u548c\u6e05\u7406\u5197\u4f59\u5c42\u3002",
      summary2Label: "\u8fd0\u884c\u65f6",
      summary2Title: "\u5185\u7f6e\u8fd0\u884c\u65f6",
      summary2Body:
        "\u5728 macOS\u3001Windows\u3001Linux \u4e0a\u542f\u52a8\u672c\u5730 VM \u5bb9\u5668\u8fd0\u884c\u65f6\uff0c\u65e0\u9700 Docker Desktop\u3002",
      summary3Label: "\u5bb9\u5668",
      summary3Title: "\u751f\u547d\u5468\u671f\u5de5\u5177",
      summary3Body:
        "\u521b\u5efa\u3001\u542f\u52a8\u3001\u505c\u6b62\u3001\u67e5\u770b\u8be6\u60c5\u3001\u6267\u884c\u547d\u4ee4\u548c\u67e5\u770b\u65e5\u5fd7\uff0c\u4e00\u5c4f\u5b8c\u6210\u3002",
      summary4Label: "CLI",
      summary4Title: "脚本化执行",
      summary4Body:
        "CLI + runtime 是最小可用单元，适合被上层工具作为受控容器执行能力调用。",
      sectionKicker: "\u4e3a\u4ec0\u4e48\u9009 CrateBay",
      sectionTitle: "\u76f8\u5bf9 Docker Desktop \u548c OrbStack \u66f4\u8f7b\u91cf\u7684\u672c\u5730\u65b9\u6848\u3002",
      sectionBody:
        "CrateBay 把日常容器工作流压实：运行时管理、镜像拉取和打包、Pod 分组、容器详情、日志和终端访问。",
      card1Title: "\u955c\u50cf\u7ba1\u7406",
      card1Body:
        "搜索 Docker Hub，配置镜像加速源，拉取镜像，打标签，导出/导入归档，打包容器，并清理本地存储。",
      card2Title: "内置运行时",
      card2Body:
        "自研 runtime 是默认路径；外部 Docker 端点只作为显式兼容覆盖。",
      card3Title: "Pod 与内置镜像",
      card3Body:
        "用受管 Docker 网络组织相关容器，并按需加载内置开发镜像。",
      card4Title: "\u8de8\u5e73\u53f0",
      card4Body:
        "macOS (Virtualization.framework)\u3001Linux (KVM)\u3001Windows (WSL2)\u3002\u57fa\u4e8e Tauri v2 \u6784\u5efa\uff0c\u539f\u751f\u6027\u80fd\u3002",
      statusKicker: "\u72b6\u6001",
      statusTitle: "v0.9 \u2014 镜像、Pod、Runtime",
      statusBody:
        "内置 runtime、内置镜像、镜像拉取/打包/导出/导入、Pod 分组、CLI 一次性执行和容器生命周期是当前重点。",
      footer: "CrateBay \u00b7 <span data-year></span>",
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
