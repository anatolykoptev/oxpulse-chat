<div class="bg-layer">
  <div class="gradient-orb orb-1"></div>
  <div class="gradient-orb orb-2"></div>
  <div class="gradient-orb orb-3"></div>
  <div class="noise-overlay"></div>
</div>

<style>
  .bg-layer {
    position: fixed;
    inset: 0;
    z-index: 0;
    overflow: hidden;
  }

  .gradient-orb {
    position: absolute;
    border-radius: 50%;
    filter: blur(120px);
    will-change: transform;
  }

  .orb-1 {
    width: 600px;
    height: 600px;
    top: -20%;
    left: -10%;
    background: radial-gradient(circle, rgba(201, 169, 110, 0.08) 0%, transparent 70%);
    animation: drift-1 25s ease-in-out infinite;
  }

  .orb-2 {
    width: 500px;
    height: 500px;
    bottom: -15%;
    right: -5%;
    background: radial-gradient(circle, rgba(201, 169, 110, 0.05) 0%, transparent 70%);
    animation: drift-2 30s ease-in-out infinite;
  }

  .orb-3 {
    width: 300px;
    height: 300px;
    top: 40%;
    left: 50%;
    transform: translateX(-50%);
    background: radial-gradient(circle, rgba(201, 169, 110, 0.04) 0%, transparent 60%);
    animation: drift-3 20s ease-in-out infinite;
  }

  @keyframes drift-1 {
    0%, 100% { transform: translate(0, 0) scale(1); }
    33% { transform: translate(40px, 30px) scale(1.05); }
    66% { transform: translate(-20px, 15px) scale(0.98); }
  }

  @keyframes drift-2 {
    0%, 100% { transform: translate(0, 0) scale(1); }
    50% { transform: translate(-30px, -20px) scale(1.03); }
  }

  @keyframes drift-3 {
    0%, 100% { transform: translateX(-50%) scale(1); opacity: 0.5; }
    50% { transform: translateX(-50%) scale(1.1); opacity: 1; }
  }

  .noise-overlay {
    position: absolute;
    inset: 0;
    opacity: 0.025;
    background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='1'/%3E%3C/svg%3E");
    background-repeat: repeat;
    background-size: 256px 256px;
    pointer-events: none;
  }
</style>
