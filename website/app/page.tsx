import Link from "next/link";
import {
  ArrowRight, ArrowUpRight, BookOpen, Box, Check, Code2, Database,
  Folder, Github, GitBranch, Globe, Layers3, Monitor, Network, Shield, Terminal,
} from "lucide-react";
import { Brand } from "@/components/brand";

const github = "https://github.com/airlock-org/airlock";
const features = [
  { icon: Box, title: "NixOS", text: "Declarative host and guest systems, built from pinned inputs." },
  { icon: Shield, title: "Qubes architecture", text: "Compartmentalized VMs with Qubes lifecycle, devices, and policy." },
  { icon: Layers3, title: "Xen", text: "The virtualization foundation, with NixOS running in dom0." },
  { icon: Network, title: "qrexec", text: "Inter-VM communication through the existing Qubes machinery." },
  { icon: GitBranch, title: "Whole-machine releases", text: "Committed configuration, coordinated deployment, and rollback." },
  { icon: Database, title: "Persistent guests", text: "Read-only NixOS system content with persistent private data." },
];
const systemDomains = [
  { name: "sys-gui", role: "GNOME desktop", icon: Monitor },
  { name: "sys-net", role: "Network devices", icon: Globe },
  { name: "sys-firewall", role: "Network policy", icon: Shield },
  { name: "sys-ctrl", role: "Build controller", icon: GitBranch },
];

export default function Home() {
  return (
    <div className="airlock-site">
      <a href="#main" className="skip-link">Skip to content</a>
      <header className="site-header">
        <div className="site-container header-inner">
          <Brand />
          <nav aria-label="Main navigation">
            <a href="#main" className="nav-home" aria-current="page">Home</a>
            <a href="#features" className="nav-section">Features</a>
            <a href="#architecture" className="nav-section">Architecture</a>
            <Link href="/docs/">Docs</Link>
            <Link href="/docs/status/">Status</Link>
          </nav>
          <div className="header-actions">
            <a href={github} className="github-link" aria-label="Airlock on GitHub"><Github size={21} aria-hidden="true" /></a>
            <Link href="/docs/getting-started/" className="site-button header-cta">Get started <ArrowRight size={15} aria-hidden="true" /></Link>
          </div>
        </div>
      </header>
      <main id="main">
        <section className="hero site-container" aria-labelledby="hero-title">
          <div className="hero-copy">
            <p className="eyebrow">DECLARATIVE / COMPARTMENTALIZED / EXPERIMENTAL</p>
            <h1 id="hero-title">Airlock<span>OS</span></h1>
            <p className="hero-tagline">Qubes architecture.<br />Declared in Nix.</p>
            <p className="hero-description">An experimental Nix-native Qubes system, built on Xen. NixOS hosts and guests, a desktop across domains, and configuration for the whole machine.</p>
            <div className="button-row">
              <Link href="/docs/getting-started/" className="site-button primary">Get started <ArrowRight size={17} aria-hidden="true" /></Link>
              <Link href="/docs/" className="site-button secondary"><BookOpen size={16} aria-hidden="true" /> Explore the docs</Link>
            </div>
            <p className="hero-note"><span className="status-dot" /> Proof of concept. Explore the source and recorded evidence.</p>
          </div>
          <figure className="hero-emblem">
            <div className="emblem-grid" aria-hidden="true" />
            <img src="/logo.svg" width="280" height="284" alt="Airlock's octagonal doorway emblem" />
            <figcaption>
              <ul aria-label="System foundations"><li>NIXOS</li><li>QUBES</li><li>XEN</li><li>QREXEC</li></ul>
              <span className="emblem-rule" />
              <p>ONE MACHINE.<br />DISTINCT DOMAINS.</p>
            </figcaption>
          </figure>
        </section>
        <section className="features site-container" id="features" aria-label="Airlock system features">
          <div className="feature-grid">
            {features.map(({ icon: Icon, title, text }) => (
              <article className="feature-card" key={title}>
                <Icon size={30} strokeWidth={1.5} aria-hidden="true" />
                <div><h2>{title}</h2><p>{text}</p></div>
              </article>
            ))}
          </div>
        </section>
        <section className="architecture-section" id="architecture" aria-labelledby="architecture-title">
          <div className="site-container architecture-inner">
            <div className="system-copy">
              <p className="eyebrow">THE SYSTEM</p>
              <h2 id="architecture-title">Compartmentalized.<br />From the hypervisor up.</h2>
              <p>Airlock brings declarative NixOS systems to the existing Qubes architecture. Xen runs the guests; Qubes retains lifecycle, communication, device ownership, and policy.</p>
              <ul className="system-checklist">
                {[
                  "NixOS in dom0 and declared guests",
                  "Closure-backed, read-only guest systems",
                  "Xen, qrexec, QubesDB, and Qubes policy",
                  "Whole-machine deployment and recovery",
                  "Persistent private data across updates",
                ].map(item => <li key={item}><Check size={13} strokeWidth={3} aria-hidden="true" />{item}</li>)}
              </ul>
              <Link href="/docs/architecture/" className="text-link">Explore the architecture <ArrowRight size={17} aria-hidden="true" /></Link>
            </div>
            <figure className="domain-diagram">
              <div className="dom0-card">
                <img src="/logo.svg" width="48" height="49" alt="" aria-hidden="true" />
                <div><strong>Dom0 <span>(NixOS / Xen)</span></strong><p>Lifecycle · Devices · Policy · Activation</p></div>
              </div>
              <div className="domain-branches" aria-hidden="true" />
              <div className="system-domains">
                {systemDomains.map(({ name, role, icon: Icon }) => (
                  <div className="domain-card" key={name}><Icon size={25} strokeWidth={1.5} aria-hidden="true" /><strong>{name}</strong><span>{role}</span></div>
                ))}
              </div>
              <div className="working-domain-label">WORKING DOMAINS</div>
              <div className="working-domains">
                <div className="domain-card"><Terminal size={24} strokeWidth={1.5} aria-hidden="true" /><strong>sys-agent</strong><span>Development access</span></div>
                <div className="domain-card"><Folder size={24} strokeWidth={1.5} aria-hidden="true" /><strong>usr-home</strong><span>User applications</span></div>
              </div>
              <figcaption>Selected domains from the current development lab. <Link href="/docs/guides/usb-lab/">Read the topology and access policy <ArrowUpRight size={12} aria-hidden="true" /></Link></figcaption>
            </figure>
          </div>
        </section>
        <section className="desktop-section site-container" aria-labelledby="desktop-title">
          <figure className="desktop-preview" aria-label="Illustrative desktop with a development terminal in sys-agent">
            <div className="desktop-topbar"><span>sys-gui</span><span>GNOME</span></div>
            <div className="desktop-wallpaper"><img src="/logo.svg" width="180" height="183" alt="" aria-hidden="true" /></div>
            <div className="desktop-dock" aria-hidden="true"><Monitor size={17} /><Terminal size={17} /><Folder size={17} /><Code2 size={17} /></div>
            <div className="terminal-window">
              <div className="terminal-title"><span className="vm-dot" /> sys-agent <span className="terminal-controls" aria-hidden="true">− &nbsp;□ &nbsp;×</span></div>
              <div className="terminal-body">
                <p><span className="terminal-prompt">~/airlock</span> <span className="terminal-branch">main</span></p>
                <pre><code><span className="terminal-comment"># Prepare dependencies explicitly</span>{"\n"}<span className="terminal-prompt">$</span> uv sync --locked{"\n\n"}<span className="terminal-comment"># Run local checks</span>{"\n"}<span className="terminal-prompt">$</span> make test-quick{"\n\n"}<span className="terminal-cursor" aria-hidden="true">▌</span></code></pre>
              </div>
            </div>
            <figcaption>Illustrative desktop layout · current physical lab uses GNOME</figcaption>
          </figure>
          <div className="desktop-copy">
            <p className="eyebrow">A DESKTOP ACROSS DOMAINS</p>
            <h2 id="desktop-title">An everyday interface.<br />Explicit system boundaries.</h2>
            <p>The physical lab runs GNOME in sys-gui, with application windows and VM identity integrated into the desktop. Begin with local checks, then follow the documented hardware workflow.</p>
            <div className="desktop-details">
              <div><Monitor size={24} strokeWidth={1.5} aria-hidden="true" /><p><strong>GNOME desktop</strong><span>Current physical lab</span></p></div>
              <div><Box size={24} strokeWidth={1.5} aria-hidden="true" /><p><strong>VM identity</strong><span>Visible application context</span></p></div>
              <div><Code2 size={24} strokeWidth={1.5} aria-hidden="true" /><p><strong>Hyprland experiment</strong><span>Physical acceptance pending</span></p></div>
              <div><BookOpen size={24} strokeWidth={1.5} aria-hidden="true" /><p><strong>Operating guides</strong><span>Deployment and recovery</span></p></div>
            </div>
            <Link href="/docs/guides/gnome/" className="text-link">Read about the desktop <ArrowRight size={17} aria-hidden="true" /></Link>
          </div>
        </section>
        <section className="status-section site-container" aria-label="Project maturity">
          <div><p className="eyebrow"><span className="status-dot" /> EXPERIMENTAL / EVIDENCE IN THE OPEN</p><p>VM lifecycle, qrexec, deployment, and rollback have recorded trials. GPU recovery and broader hardware support remain open. Qubes OS security equivalence has not been established; the USB lab includes privileged development access.</p></div>
          <Link href="/docs/status/" className="site-button secondary">Validation status <ArrowRight size={16} aria-hidden="true" /></Link>
        </section>
      </main>
      <footer className="site-footer">
        <div className="site-container footer-inner">
          <Brand /><p>An experimental Nix-native Qubes system.</p>
          <div><Link href="/docs/">Docs</Link><Link href="/docs/reference/third-party/">Licensing</Link><Link href="/docs/reference/security/">Security</Link><a href={github} aria-label="Airlock source on GitHub"><Github size={19} aria-hidden="true" /></a></div>
        </div>
      </footer>
    </div>
  );
}
