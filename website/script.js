/* ============================================================
   CargoBay — Official Website Scripts
   ============================================================ */

(function () {
  'use strict';

  // ----------------------------------------------------------
  // i18n — Bilingual content (English / Chinese)
  // ----------------------------------------------------------
  const i18n = {
    en: {
      // Navbar
      navFeatures: 'Features',
      navDemo: 'Demo',
      navTech: 'Tech Stack',
      navCompare: 'Compare',
      navInstall: 'Install',

      // Hero
      heroBadge: 'v0.1.0 Pre-release',
      heroTitle1: 'Cargo',
      heroTitle2: 'Bay',
      heroTagline:
        'A lightweight, open-source desktop app for managing Docker containers and Linux VMs. The fast alternative to Docker Desktop.',
      heroDownload: 'Download for macOS',
      heroGithub: 'View on GitHub',
      statStars: 'GitHub Stars',
      statLang: 'Pure Rust',
      statLicense: 'Apache 2.0',

      // Features
      featLabel: 'Features',
      featTitle: 'Everything you need to manage containers.',
      featDesc:
        'CargoBay brings a unified, native-speed experience for Docker containers and Linux VMs — without the overhead.',
      feat1Title: 'Docker Container Management',
      feat1Desc:
        'Full lifecycle control — create, start, stop, restart, remove. Real-time log streaming and exec into running containers.',
      feat2Title: 'Linux Virtual Machines',
      feat2Desc:
        'Native VM support via Virtualization.framework (macOS), KVM (Linux), and Hyper-V (Windows). Boot in seconds.',
      feat3Title: 'Image Registry Search',
      feat3Desc:
        'Search and pull images from Docker Hub, Quay.io, and private registries. Browse tags, inspect layers.',
      feat4Title: 'Resource Monitoring',
      feat4Desc:
        'Real-time CPU, memory, network, and disk I/O monitoring for every container and VM. Spot issues instantly.',
      feat5Title: 'Port Forwarding & VirtioFS',
      feat5Desc:
        'Automatic port forwarding and near-native file sharing between host and guest via VirtioFS.',
      feat6Title: 'Cross-platform',
      feat6Desc:
        'Runs on macOS, Linux, and Windows. Same interface, same speed, same experience on every platform.',

      // Demo
      demoLabel: 'Interface',
      demoTitle: 'Built for speed and clarity.',
      demoDesc:
        'A clean, responsive GUI built with Tauri and React. Every interaction feels instant.',
      demoTitlebar: 'CargoBay',
      demoSidebar1: 'Containers',
      demoSidebar2: 'Images',
      demoSidebar3: 'Volumes',
      demoSidebar4: 'Networks',
      demoSidebar5: 'VMs',
      demoHeader: 'Containers',
      demoBtn: '+ New',
      demoThName: 'Name',
      demoThImage: 'Image',
      demoThStatus: 'Status',
      demoThCpu: 'CPU',
      demoThMem: 'Memory',
      demoR1Name: 'web-frontend',
      demoR1Img: 'nginx:alpine',
      demoR1Status: 'Running',
      demoR1Cpu: '0.3%',
      demoR1Mem: '24 MB',
      demoR2Name: 'api-server',
      demoR2Img: 'node:20-slim',
      demoR2Status: 'Running',
      demoR2Cpu: '1.2%',
      demoR2Mem: '128 MB',
      demoR3Name: 'db-postgres',
      demoR3Img: 'postgres:16',
      demoR3Status: 'Stopped',
      demoR3Cpu: '—',
      demoR3Mem: '—',
      demoR4Name: 'redis-cache',
      demoR4Img: 'redis:7-alpine',
      demoR4Status: 'Paused',
      demoR4Cpu: '—',
      demoR4Mem: '52 MB',

      // Tech stack
      techLabel: 'Architecture',
      techTitle: 'Built with Rust. Powered by gRPC.',
      techDesc:
        'Full-stack Rust — from the GUI backend to the daemon to the VM engine. Zero garbage collection. Maximum performance.',

      // Comparison
      compLabel: 'Comparison',
      compTitle: 'How CargoBay stacks up.',
      compDesc: 'A quick look at how CargoBay compares to the alternatives.',
      compFeature: 'Feature',
      compCargoBay: 'CargoBay',
      compDocker: 'Docker Desktop',
      compOrbStack: 'OrbStack',
      compAppSize: 'App Size',
      compMemory: 'Memory Usage',
      compStartup: 'Container Startup',
      compOpenSource: 'Open Source',
      compFree: 'Free',
      compVM: 'VM Support',
      compCross: 'Cross-platform',

      compCBSize: '~20 MB',
      compDDSize: '~1.5 GB',
      compOSSize: '~200 MB',
      compCBMem: '~50 MB',
      compDDMem: '~2 GB',
      compOSMem: '~200 MB',
      compCBStart: '< 1s',
      compDDStart: '~10s',
      compOSStart: '~2s',

      // Getting started
      installLabel: 'Getting Started',
      installTitle: 'Install in seconds.',
      installDesc: 'Choose your preferred installation method.',
      installBrew: 'Homebrew',
      installBrewDesc: 'macOS & Linux',
      installCargo: 'Cargo',
      installCargoDesc: 'Build from source',
      installDirect: 'Direct Download',
      installDirectDesc: 'Pre-built binary',

      // Footer
      footerDocs: 'Documentation',
      footerRoadmap: 'Roadmap',
      footerLicense: 'License',
    },
    zh: {
      navFeatures: '功能',
      navDemo: '演示',
      navTech: '技术栈',
      navCompare: '对比',
      navInstall: '安装',

      heroBadge: 'v0.1.0 预览版',
      heroTitle1: 'Cargo',
      heroTitle2: 'Bay',
      heroTagline:
        '轻量级开源桌面应用，统一管理 Docker 容器和 Linux 虚拟机。Docker Desktop 的快速替代方案。',
      heroDownload: '下载 macOS 版本',
      heroGithub: '在 GitHub 查看',
      statStars: 'GitHub Stars',
      statLang: '纯 Rust',
      statLicense: 'Apache 2.0',

      featLabel: '核心功能',
      featTitle: '容器管理，一应俱全。',
      featDesc:
        'CargoBay 为 Docker 容器和 Linux 虚拟机提供统一、原生速度的管理体验——零额外开销。',
      feat1Title: 'Docker 容器管理',
      feat1Desc:
        '全生命周期控制——创建、启动、停止、重启、删除。实时日志流和进入运行中容器的终端。',
      feat2Title: 'Linux 虚拟机',
      feat2Desc:
        '通过 Virtualization.framework (macOS)、KVM (Linux)、Hyper-V (Windows) 原生支持虚拟机，秒级启动。',
      feat3Title: '镜像仓库搜索',
      feat3Desc:
        '从 Docker Hub、Quay.io 和私有仓库搜索和拉取镜像。浏览标签、检查层信息。',
      feat4Title: '资源监控',
      feat4Desc:
        '实时监控每个容器和虚拟机的 CPU、内存、网络和磁盘 I/O。即时发现问题。',
      feat5Title: '端口转发 & VirtioFS',
      feat5Desc:
        '自动端口转发，通过 VirtioFS 实现主机与客户机之间的近原生文件共享。',
      feat6Title: '跨平台',
      feat6Desc:
        '支持 macOS、Linux 和 Windows。相同的界面、相同的速度、相同的体验。',

      demoLabel: '界面',
      demoTitle: '为速度和清晰度而生。',
      demoDesc:
        '基于 Tauri 和 React 构建的简洁响应式 GUI。每次交互都瞬间完成。',
      demoTitlebar: 'CargoBay',
      demoSidebar1: '容器',
      demoSidebar2: '镜像',
      demoSidebar3: '卷',
      demoSidebar4: '网络',
      demoSidebar5: '虚拟机',
      demoHeader: '容器',
      demoBtn: '+ 新建',
      demoThName: '名称',
      demoThImage: '镜像',
      demoThStatus: '状态',
      demoThCpu: 'CPU',
      demoThMem: '内存',
      demoR1Name: 'web-frontend',
      demoR1Img: 'nginx:alpine',
      demoR1Status: '运行中',
      demoR1Cpu: '0.3%',
      demoR1Mem: '24 MB',
      demoR2Name: 'api-server',
      demoR2Img: 'node:20-slim',
      demoR2Status: '运行中',
      demoR2Cpu: '1.2%',
      demoR2Mem: '128 MB',
      demoR3Name: 'db-postgres',
      demoR3Img: 'postgres:16',
      demoR3Status: '已停止',
      demoR3Cpu: '—',
      demoR3Mem: '—',
      demoR4Name: 'redis-cache',
      demoR4Img: 'redis:7-alpine',
      demoR4Status: '已暂停',
      demoR4Cpu: '—',
      demoR4Mem: '52 MB',

      techLabel: '架构',
      techTitle: 'Rust 构建，gRPC 驱动。',
      techDesc:
        '全栈 Rust——从 GUI 后端到守护进程再到 VM 引擎。零垃圾回收，极致性能。',

      compLabel: '对比',
      compTitle: 'CargoBay 横向对比。',
      compDesc: '快速了解 CargoBay 与同类产品的差异。',
      compFeature: '特性',
      compCargoBay: 'CargoBay',
      compDocker: 'Docker Desktop',
      compOrbStack: 'OrbStack',
      compAppSize: '应用大小',
      compMemory: '内存占用',
      compStartup: '容器启动',
      compOpenSource: '开源',
      compFree: '免费',
      compVM: '虚拟机支持',
      compCross: '跨平台',

      compCBSize: '~20 MB',
      compDDSize: '~1.5 GB',
      compOSSize: '~200 MB',
      compCBMem: '~50 MB',
      compDDMem: '~2 GB',
      compOSMem: '~200 MB',
      compCBStart: '< 1s',
      compDDStart: '~10s',
      compOSStart: '~2s',

      installLabel: '快速开始',
      installTitle: '秒级安装。',
      installDesc: '选择你喜欢的安装方式。',
      installBrew: 'Homebrew',
      installBrewDesc: 'macOS & Linux',
      installCargo: 'Cargo',
      installCargoDesc: '从源码构建',
      installDirect: '直接下载',
      installDirectDesc: '预编译二进制',

      footerDocs: '文档',
      footerRoadmap: '路线图',
      footerLicense: '许可证',
    },
  };

  let currentLang = 'en';

  function setLang(lang) {
    currentLang = lang;
    document.documentElement.lang = lang === 'zh' ? 'zh-CN' : 'en';

    // Update toggle buttons
    document.querySelectorAll('.lang-toggle button').forEach((btn) => {
      btn.classList.toggle('active', btn.dataset.lang === lang);
    });

    // Update all [data-i18n] elements
    document.querySelectorAll('[data-i18n]').forEach((el) => {
      const key = el.getAttribute('data-i18n');
      if (i18n[lang][key]) {
        el.textContent = i18n[lang][key];
      }
    });

    // Update [data-i18n-html] elements (for innerHTML)
    document.querySelectorAll('[data-i18n-html]').forEach((el) => {
      const key = el.getAttribute('data-i18n-html');
      if (i18n[lang][key]) {
        el.innerHTML = i18n[lang][key];
      }
    });
  }

  // ----------------------------------------------------------
  // Navbar scroll effect
  // ----------------------------------------------------------
  const navbar = document.querySelector('.navbar');
  function onScroll() {
    if (window.scrollY > 40) {
      navbar.classList.add('scrolled');
    } else {
      navbar.classList.remove('scrolled');
    }
  }
  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  // ----------------------------------------------------------
  // Mobile menu
  // ----------------------------------------------------------
  const mobileBtn = document.querySelector('.mobile-menu-btn');
  const navLinks = document.querySelector('.nav-links');
  if (mobileBtn) {
    mobileBtn.addEventListener('click', () => {
      navLinks.classList.toggle('open');
    });
    // Close on link click
    navLinks.querySelectorAll('a').forEach((a) => {
      a.addEventListener('click', () => {
        navLinks.classList.remove('open');
      });
    });
  }

  // ----------------------------------------------------------
  // Language toggle
  // ----------------------------------------------------------
  document.querySelectorAll('.lang-toggle button').forEach((btn) => {
    btn.addEventListener('click', () => {
      setLang(btn.dataset.lang);
    });
  });

  // ----------------------------------------------------------
  // Copy code blocks
  // ----------------------------------------------------------
  document.querySelectorAll('.copy-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      const block = btn.closest('.code-block');
      const code = block.querySelector('code').textContent;
      navigator.clipboard.writeText(code.replace(/^\$ /gm, '')).then(() => {
        const original = btn.textContent;
        btn.textContent = 'Copied!';
        setTimeout(() => {
          btn.textContent = original;
        }, 1500);
      });
    });
  });

  // ----------------------------------------------------------
  // Scroll reveal (IntersectionObserver)
  // ----------------------------------------------------------
  const reveals = document.querySelectorAll('.reveal');
  if ('IntersectionObserver' in window) {
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            entry.target.classList.add('visible');
            observer.unobserve(entry.target);
          }
        });
      },
      { threshold: 0.12 }
    );
    reveals.forEach((el) => observer.observe(el));
  } else {
    reveals.forEach((el) => el.classList.add('visible'));
  }

  // ----------------------------------------------------------
  // Grid / particle canvas background
  // ----------------------------------------------------------
  const canvas = document.getElementById('grid-canvas');
  if (canvas) {
    const ctx = canvas.getContext('2d');
    let w, h;
    let particles = [];
    const PARTICLE_COUNT = 60;
    const GRID_SIZE = 60;
    const CONNECTION_DIST = 140;

    function resize() {
      w = canvas.width = window.innerWidth;
      h = canvas.height = window.innerHeight;
    }

    function initParticles() {
      particles = [];
      for (let i = 0; i < PARTICLE_COUNT; i++) {
        particles.push({
          x: Math.random() * w,
          y: Math.random() * h,
          vx: (Math.random() - 0.5) * 0.3,
          vy: (Math.random() - 0.5) * 0.3,
          r: Math.random() * 1.5 + 0.5,
        });
      }
    }

    function drawGrid() {
      ctx.strokeStyle = 'rgba(42, 45, 69, 0.25)';
      ctx.lineWidth = 0.5;
      for (let x = 0; x < w; x += GRID_SIZE) {
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
        ctx.stroke();
      }
      for (let y = 0; y < h; y += GRID_SIZE) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
        ctx.stroke();
      }
    }

    function drawParticles() {
      for (let i = 0; i < particles.length; i++) {
        const p = particles[i];
        // Move
        p.x += p.vx;
        p.y += p.vy;
        if (p.x < 0) p.x = w;
        if (p.x > w) p.x = 0;
        if (p.y < 0) p.y = h;
        if (p.y > h) p.y = 0;

        // Draw dot
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
        ctx.fillStyle = 'rgba(139, 92, 246, 0.5)';
        ctx.fill();

        // Connections
        for (let j = i + 1; j < particles.length; j++) {
          const q = particles[j];
          const dx = p.x - q.x;
          const dy = p.y - q.y;
          const dist = Math.sqrt(dx * dx + dy * dy);
          if (dist < CONNECTION_DIST) {
            ctx.beginPath();
            ctx.moveTo(p.x, p.y);
            ctx.lineTo(q.x, q.y);
            ctx.strokeStyle = `rgba(139, 92, 246, ${
              0.12 * (1 - dist / CONNECTION_DIST)
            })`;
            ctx.lineWidth = 0.5;
            ctx.stroke();
          }
        }
      }
    }

    function animate() {
      ctx.clearRect(0, 0, w, h);
      drawGrid();
      drawParticles();
      requestAnimationFrame(animate);
    }

    resize();
    initParticles();
    animate();

    window.addEventListener('resize', () => {
      resize();
      initParticles();
    });
  }

  // Initialize language
  setLang('en');
})();
