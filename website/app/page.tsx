import Link from "next/link";
import { ArrowDown, ArrowRight, ArrowUpRight, BookOpen, GitBranch, Layers3, Terminal } from "lucide-react";
import { Brand } from "@/components/brand";

const github = "https://github.com/airlock-org/airlock";

export default function Home() {
  return (
    <div className="airlock-site">
      <a href="#main" className="skip-link">Skip to content</a>
      <header className="site-header">
        <div className="site-container header-inner">
          <Brand />
          <nav aria-label="Main navigation">
            <Link href="/docs/">Docs</Link>
            <Link href="/docs/status/">Status</Link>
            <a href={github}>GitHub <ArrowUpRight size={14} aria-hidden="true" /></a>
          </nav>
        </div>
      </header>
      <main id="main">
        <section className="hero site-container" aria-labelledby="hero-title">
          <div className="hero-copy">
            <p className="eyebrow"><span className="status-dot" /> EXPERIMENTAL / NIX-NATIVE QUBES</p>
            <h1 id="hero-title">Qubes architecture.<br /><span>Declared in Nix.</span></h1>
            <p className="hero-description">NixOS hosts and guests, built on Xen and the Qubes machinery. Explore a declarative approach to the whole machine.</p>
            <div className="button-row">
              <Link href="/docs/getting-started/" className="site-button primary">Start exploring <ArrowRight size={18} aria-hidden="true" /></Link>
              <Link href="/docs/architecture/" className="site-button secondary">Read the architecture</Link>
            </div>
            <p className="hero-note">Proof of concept. Hardware evidence is limited to the named lab.</p>
          </div>
          <figure className="machine-diagram" aria-label="Declaration layers: Nix defines a whole-machine release, which supplies a NixOS Xen host and declared NixOS guests. This is a conceptual diagram, not network routing.">
            <div className="diagram-caption"><span>ONE MACHINE / DECLARED SYSTEMS</span><Layers3 size={17} aria-hidden="true" /></div>
            <div className="declaration"><span className="diagram-number">01</span><div><strong>Nix declarations</strong><span>Host + guest configurations</span></div><code>.nix</code></div>
            <div className="diagram-connector"><ArrowDown size={17} aria-hidden="true" /><span>build &amp; release</span></div>
            <div className="host-layer"><span className="diagram-number">02</span><div><strong>NixOS dom0</strong><span>Xen · Qubes lifecycle · policy</span></div><span className="host-tag">HOST</span></div>
            <div className="diagram-connector"><ArrowDown size={17} aria-hidden="true" /><span>declared NixOS guests</span></div>
            <div className="guest-grid">
              <div className="guest network"><span className="guest-dot" /><strong>sys-net</strong><span>Network devices</span></div>
              <div className="guest desktop"><span className="guest-dot" /><strong>sys-gui</strong><span>GNOME desktop</span></div>
              <div className="guest development"><span className="guest-dot" /><strong>sys-agent</strong><span>Development</span></div>
            </div>
            <figcaption>Selected guests from the development lab. <Link href="/docs/guides/usb-lab/">See full topology <ArrowUpRight size={12} aria-hidden="true" /></Link></figcaption>
          </figure>
        </section>
        <div className="mechanisms-band">
          <div className="site-container mechanisms-inner"><span>BUILT ON QUBES</span><ul aria-label="Preserved Qubes mechanisms">{["Xen", "qrexec", "QubesDB", "qubesd", "libvirt", "Qubes policy"].map(name => <li key={name}>{name}</li>)}</ul></div>
        </div>
        <section className="approach site-container" aria-labelledby="approach-title">
          <div className="section-heading"><p className="eyebrow">THE APPROACH</p><h2 id="approach-title">Configuration belongs<br />with the system.</h2><p>Declare the host and its guests, produce a release, and verify the machine that actually runs.</p></div>
          <div className="approach-list">
            <article><span className="row-number">01</span><div><h3>Declare hosts and guests together</h3><p>Nix describes system configuration and pinned build inputs. Qubes retains VM lifecycle, communication, and policy.</p></div></article>
            <article><span className="row-number">02</span><div><h3>Deploy a whole-machine release</h3><p>Committed source flows through the controller to host-owned admission and activation, with retained recovery generations.</p></div></article>
            <article><span className="row-number">03</span><div><h3>Record what has been verified</h3><p>Local checks, nested Xen trials, and physical acceptance carry separate evidence. A running generation is verified after deployment.</p></div></article>
          </div>
        </section>
        <section className="lab-section" aria-labelledby="lab-title">
          <div className="site-container lab-inner">
            <div><p className="eyebrow">FROM THE PHYSICAL LAB</p><h2 id="lab-title">Working paths.<br />Explicit limits.</h2><p>The current physical example runs GNOME in sys-gui on a ThinkPad T14 Gen 3 AMD. Every acceptance record has a configuration and scope.</p><Link href="/docs/status/" className="text-link">Read the validation status <ArrowRight size={17} aria-hidden="true" /></Link></div>
            <div className="lab-records">
              <div className="record"><span className="record-label">DEMONSTRATED</span><ul><li>VM lifecycle and qrexec</li><li>Closure-backed NixOS guests</li><li>Whole-machine deployment and rollback</li><li>Shared storage and persistent guest data</li></ul></div>
              <div className="record open"><span className="record-label">OPEN WORK</span><p>GPU reset and restart, GUI recovery, first-run acceptance, and broader hardware support.</p></div>
            </div>
          </div>
          <div className="site-container maturity-note"><span className="status-dot" /><p>Airlock is experimental. Qubes OS security equivalence has not been established. The USB lab includes privileged development access.</p></div>
        </section>
        <section className="explore site-container" aria-labelledby="explore-title">
          <div className="explore-heading"><div><p className="eyebrow">EXPLORE THE PROJECT</p><h2 id="explore-title">Start with the source.</h2></div><p>Local checks are the first step.<br />Hardware work follows the documented lab workflow.</p></div>
          <div className="explore-grid">
            <Link href="/docs/getting-started/" className="explore-link"><Terminal size={23} aria-hidden="true" /><h3>Getting started</h3><p>Prepare a checkout and run the quick suite.</p><span>Start here <ArrowRight size={17} aria-hidden="true" /></span></Link>
            <Link href="/docs/guides/machine-deployment/" className="explore-link"><GitBranch size={23} aria-hidden="true" /><h3>Machine workflow</h3><p>Build, plan, apply, verify, and recover.</p><span>Read the guide <ArrowRight size={17} aria-hidden="true" /></span></Link>
            <Link href="/docs/" className="explore-link"><BookOpen size={23} aria-hidden="true" /><h3>Documentation</h3><p>Architecture, operating guides, plans, and evidence.</p><span>Browse the docs <ArrowRight size={17} aria-hidden="true" /></span></Link>
          </div>
        </section>
      </main>
      <footer className="site-footer"><div className="site-container footer-inner"><Brand /><p>An experimental Nix-native Qubes system.</p><div><a href={github}>Source <ArrowUpRight size={13} aria-hidden="true" /></a><Link href="/docs/reference/third-party/">Licensing</Link><Link href="/docs/reference/security/">Security</Link></div></div></footer>
    </div>
  );
}
