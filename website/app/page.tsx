import Link from "next/link";
import {
  ArrowRight, ArrowUpRight, BookOpen, Box, Check, Code2, Database,
  Folder, Github, GitBranch, Globe, Layers3, Monitor, Network, Shield, Terminal,
} from "lucide-react";
import { Brand } from "@/components/brand";
import { MobileNav } from "@/components/mobile-nav";

const github = "https://github.com/airlock-org/airlock";
const features = [
  { icon: Box, title: "NixOS", text: "Declare host and guest systems together, with pinned build inputs.", href: "/docs/architecture/" },
  { icon: Shield, title: "Qubes architecture", text: "Keep Qubes lifecycle, device ownership, and policy across domains.", href: "/docs/architecture/" },
  { icon: Layers3, title: "Xen", text: "Run NixOS in dom0 on the Qubes virtualization foundation.", href: "/docs/architecture/" },
  { icon: Network, title: "qrexec", text: "Communicate between VMs through the existing Qubes machinery.", href: "/docs/architecture/" },
  { icon: GitBranch, title: "Machine releases", text: "Deploy committed configuration with verification and rollback.", href: "/docs/guides/machine-deployment/" },
  { icon: Database, title: "Persistent guests", text: "Read-only NixOS systems. Private data that survives updates.", href: "/docs/guides/build-store/" },
];
const systemDomains = [
  { name: "sys-gui", role: "GNOME desktop", icon: Monitor, href: "/docs/guides/gnome/" },
  { name: "sys-net", role: "Network devices", icon: Globe, href: "/docs/architecture/#network-routing" },
  { name: "sys-firewall", role: "Network policy", icon: Shield, href: "/docs/architecture/#network-routing" },
  { name: "sys-ctrl", role: "Build controller", icon: GitBranch, href: "/docs/guides/machine-deployment/" },
];

export default function Home() {
  return (
    <div className="airlock-site">
      <a href="#main" className="skip-link">Skip to content</a>
      <header className="site-header">
        <div className="site-container header-inner">
          <Brand />
          <nav className="desktop-nav" aria-label="Main navigation">
            <a href="#main" className="nav-home" aria-current="page">Home</a>
            <a href="#features" className="nav-section">Features</a>
            <a href="#architecture" className="nav-section">Architecture</a>
            <Link href="/docs/">Docs</Link>
            <Link href="/docs/status/">Status</Link>
          </nav>
          <div className="header-actions">
            <Link href="/docs/" className="mobile-docs">Docs</Link>
            <a href={github} className="github-link" aria-label="Airlock on GitHub"><Github size={21} aria-hidden="true" /></a>
            <Link href="/docs/getting-started/" className="site-button header-cta">Get started <ArrowRight size={15} aria-hidden="true" /></Link>
            <MobileNav />
          </div>
        </div>
      </header>
      <main id="main" tabIndex={-1}>
        <section className="hero site-container" aria-labelledby="hero-title">
          <div className="hero-copy">
            <p className="eyebrow hero-eyebrow"><span className="hero-stage">Experimental</span><span>NIX-NATIVE / QUBES</span></p>
            <h1 id="hero-title">Airlock<span>OS</span></h1>
            <p className="hero-tagline">Qubes architecture.<br />Declared in Nix.</p>
            <p className="hero-description">NixOS in dom0. Declarative guests on Xen. A desktop across domains, with one configuration for the whole machine.</p>
            <div className="button-row">
              <Link href="/docs/getting-started/" className="site-button primary">Get started <ArrowRight size={17} aria-hidden="true" /></Link>
              <Link href="/docs/" className="site-button secondary"><BookOpen size={16} aria-hidden="true" /> Explore the docs</Link>
            </div>
            <Link href="/docs/status/" className="hero-note"><span className="status-dot" /> See what has been verified <ArrowRight size={13} aria-hidden="true" /></Link>
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
        <section className="features site-container" id="features" aria-labelledby="features-title">
          <h2 id="features-title" className="sr-only">System foundations</h2>
          <div className="feature-grid">
            {features.map(({ icon: Icon, title, text, href }) => (
              <Link href={href} className="feature-card" key={title}>
                <Icon size={30} strokeWidth={1.5} aria-hidden="true" />
                <div><h3>{title}</h3><p>{text}</p></div>
                <ArrowUpRight size={15} className="feature-arrow" aria-hidden="true" />
              </Link>
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
              <div className="diagram-heading"><span>DECLARED DOMAINS</span></div>
              <div className="dom0-card">
                <img src="/logo.svg" width="48" height="49" alt="" aria-hidden="true" />
                <div><strong>Dom0 <span>(NixOS / Xen)</span></strong><p>Lifecycle · Devices · Policy · Activation</p></div>
              </div>
              <div className="domain-branches" aria-hidden="true" />
              <div className="system-domains">
                {systemDomains.map(({ name, role, icon: Icon, href }) => (
                  <Link href={href} className="domain-card" key={name}><Icon size={25} strokeWidth={1.5} aria-hidden="true" /><strong>{name}</strong><span>{role}</span></Link>
                ))}
              </div>
              <div className="working-domain-label">WORKING DOMAINS</div>
              <div className="working-domains">
                <Link href="/docs/guides/usb-lab/" className="domain-card"><Terminal size={24} strokeWidth={1.5} aria-hidden="true" /><strong>sys-agent</strong><span>Development access</span></Link>
                <Link href="/docs/evidence/live-vm-additions/" className="domain-card"><Folder size={24} strokeWidth={1.5} aria-hidden="true" /><strong>usr-home</strong><span>User applications</span></Link>
              </div>
              <figcaption>VM roles and responsibilities. <Link href="/docs/architecture/#vm-topology">Explore the topology <ArrowUpRight size={12} aria-hidden="true" /></Link></figcaption>
            </figure>
          </div>
        </section>
        <section className="desktop-section site-container" aria-labelledby="desktop-title">
          <figure className="desktop-preview" aria-label="Illustrative desktop with a development terminal in sys-agent">
            <div className="desktop-topbar"><span>sys-gui</span><span>GNOME</span></div>
            <div className="desktop-wallpaper"><img src="/logo.svg" width="180" height="183" alt="" aria-hidden="true" /></div>
            <div className="desktop-dock" aria-hidden="true"><Monitor size={17} /><Terminal size={17} /><Folder size={17} /><Code2 size={17} /></div>
            <div className="terminal-window">
              <div className="terminal-title"><span className="vm-dot" /> sys-agent — Terminal <span className="terminal-controls" aria-hidden="true">− &nbsp;□ &nbsp;×</span></div>
              <div className="terminal-body">
                <p><span className="terminal-prompt">~/airlock</span> <span className="terminal-branch">main</span></p>
                <pre><code><span className="terminal-comment"># Prepare dependencies</span>{"\n"}<span className="terminal-prompt">$</span> uv sync --locked{"\n\n"}<span className="terminal-comment"># Run local checks</span>{"\n"}<span className="terminal-prompt">$</span> make test-quick{"\n\n"}<span className="terminal-cursor" aria-hidden="true">▌</span></code></pre>
              </div>
            </div>
            <figcaption>Desktop layout illustration</figcaption>
          </figure>
          <div className="desktop-copy">
            <p className="eyebrow">A DESKTOP ACROSS DOMAINS</p>
            <h2 id="desktop-title">A familiar desktop.<br />Across separate domains.</h2>
            <p>GNOME runs in sys-gui. Application windows carry their VM context; system changes follow the documented deployment and recovery workflow.</p>
            <div className="desktop-details">
              <Link href="/docs/guides/gnome/"><Monitor size={24} strokeWidth={1.5} aria-hidden="true" /><p><strong>GNOME desktop</strong><span>Runs in sys-gui</span></p></Link>
              <Link href="/docs/guides/gnome/"><Box size={24} strokeWidth={1.5} aria-hidden="true" /><p><strong>VM identity</strong><span>Application context</span></p></Link>
              <Link href="/docs/guides/usb-lab/"><Terminal size={24} strokeWidth={1.5} aria-hidden="true" /><p><strong>Development access</strong><span>Through sys-agent</span></p></Link>
              <Link href="/docs/guides/machine-deployment/"><BookOpen size={24} strokeWidth={1.5} aria-hidden="true" /><p><strong>Recovery guides</strong><span>Deployment and rollback</span></p></Link>
            </div>
            <Link href="/docs/guides/gnome/" className="text-link">Read about the desktop <ArrowRight size={17} aria-hidden="true" /></Link>
          </div>
        </section>
        <section className="status-section site-container" aria-labelledby="status-title">
          <div className="status-panel">
            <div><p className="eyebrow"><span className="status-dot" /> PROJECT STATUS</p><h2 id="status-title">Experimental. With recorded trials.</h2><p>VM lifecycle, qrexec, deployment, and rollback have recorded evidence. GPU recovery and broader hardware support remain open. Development access includes a privileged dom0 shell; Qubes OS security equivalence has not been established.</p></div>
            <Link href="/docs/status/" className="site-button secondary">Validation status <ArrowRight size={16} aria-hidden="true" /></Link>
          </div>
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
