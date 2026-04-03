<script lang="ts">
  const LETTERS = 'ABCDEFGHJKLMNPQRSTUVWXYZ';
  const DIGITS = '0123456789';
  const TOTAL = 8; // 4 letters + 4 digits

  let { value = $bindable(''), onsubmit, oninput: onInputCb }: {
    value: string;
    onsubmit?: () => void;
    oninput?: (code: string) => void;
  } = $props();

  let cells: HTMLInputElement[] = [];
  let focused = $state(-1);

  function charSet(i: number): string {
    return i < 4 ? LETTERS : DIGITS;
  }

  function chars(): string[] {
    const raw = value.replace(/[^A-Z0-9]/gi, '').toUpperCase();
    return Array.from({ length: TOTAL }, (_, i) => raw[i] ?? '');
  }

  function setChar(i: number, ch: string) {
    const arr = chars();
    arr[i] = ch;
    const joined = arr.join('');
    value = joined.length > 4
      ? joined.slice(0, 4) + '-' + joined.slice(4)
      : joined;
    onInputCb?.(value);
  }

  function handleInput(i: number, e: Event) {
    const el = e.target as HTMLInputElement;
    const raw = el.value.toUpperCase().replace(/[^A-Z0-9]/g, '');
    if (!raw) return;
    const ch = raw[raw.length - 1];
    if (!charSet(i).includes(ch)) { el.value = ''; return; }
    setChar(i, ch);
    if (i < TOTAL - 1) cells[i + 1]?.focus();
    else if (i === TOTAL - 1) onsubmit?.();
  }

  function handleKeydown(i: number, e: KeyboardEvent) {
    if (e.key === 'Backspace') {
      e.preventDefault();
      if (chars()[i]) {
        setChar(i, '');
      } else if (i > 0) {
        setChar(i - 1, '');
        cells[i - 1]?.focus();
      }
    } else if (e.key === 'ArrowLeft' && i > 0) {
      cells[i - 1]?.focus();
    } else if (e.key === 'ArrowRight' && i < TOTAL - 1) {
      cells[i + 1]?.focus();
    } else if (e.key === 'Enter') {
      onsubmit?.();
    }
  }

  function handlePaste(e: ClipboardEvent) {
    e.preventDefault();
    const text = (e.clipboardData?.getData('text') ?? '').toUpperCase().replace(/[^A-Z0-9]/g, '');
    if (!text) return;
    const arr = chars();
    for (let j = 0; j < Math.min(text.length, TOTAL); j++) {
      if (charSet(j).includes(text[j])) arr[j] = text[j];
    }
    const joined = arr.join('');
    value = joined.length > 4
      ? joined.slice(0, 4) + '-' + joined.slice(4)
      : joined;
    onInputCb?.(value);
    const next = Math.min(text.length, TOTAL - 1);
    cells[next]?.focus();
    if (text.length >= TOTAL) onsubmit?.();
  }

  function handleFocus(i: number, e: FocusEvent) {
    focused = i;
    // Prevent iOS scroll-into-view jump
    e.preventDefault();
    (e.target as HTMLInputElement)?.scrollIntoView?.({ block: 'nearest' });
  }
  function handleBlur() { focused = -1; }
</script>

<div class="code-input" class:has-focus={focused >= 0} role="group" aria-label="Room code">
  {#each { length: TOTAL } as _, i}
    {#if i === 4}
      <span class="dash" aria-hidden="true">–</span>
    {/if}
    <input
      bind:this={cells[i]}
      class="cell"
      class:filled={!!chars()[i]}
      class:active={focused === i}
      type="text"
      inputmode={i < 4 ? 'text' : 'numeric'}
      maxlength="2"
      value={chars()[i]}
      placeholder={i < 4 ? 'A' : '0'}
      autocomplete="off"
      autocapitalize="characters"
      spellcheck="false"
      aria-label={`${i < 4 ? 'Letter' : 'Digit'} ${(i < 4 ? i : i - 4) + 1}`}
      oninput={(e: Event) => handleInput(i, e)}
      onkeydown={(e: KeyboardEvent) => handleKeydown(i, e)}
      onfocus={(e: FocusEvent) => handleFocus(i, e)}
      onblur={handleBlur}
      onpaste={handlePaste}
    />
  {/each}
</div>

<style>
  .code-input {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .cell {
    width: 40px;
    height: 48px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.07);
    color: #fff;
    font-family: 'Martian Mono', 'JetBrains Mono', monospace;
    font-size: 18px;
    font-weight: 500;
    text-align: center;
    outline: none;
    caret-color: transparent;
    transition: all 0.2s ease;
    padding: 0;
  }

  .cell::placeholder {
    color: rgba(255, 255, 255, 0.25);
    font-weight: 400;
  }

  .cell:focus, .cell.active {
    border-color: rgba(201, 169, 110, 0.5);
    background: rgba(201, 169, 110, 0.06);
    box-shadow: 0 0 0 3px rgba(201, 169, 110, 0.08);
  }

  .cell.filled {
    border-color: rgba(255, 255, 255, 0.22);
    background: rgba(255, 255, 255, 0.1);
  }

  .dash {
    color: rgba(255, 255, 255, 0.35);
    font-size: 20px;
    font-weight: 300;
    margin: 0 2px;
    user-select: none;
  }

  @media (max-width: 480px) {
    .cell {
      width: 36px;
      height: 44px;
      font-size: 16px;
      border-radius: 8px;
    }
    .code-input { gap: 4px; }
    .dash { margin: 0 1px; font-size: 18px; }
  }

  @media (max-height: 600px) {
    .cell {
      width: 32px;
      height: 38px;
      font-size: 14px;
      border-radius: 7px;
    }
    .code-input { gap: 3px; }
    .dash { font-size: 16px; }
  }
</style>
