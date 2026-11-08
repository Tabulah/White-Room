<script lang="ts">
  import { getAppInfo } from './lib/api/app';

  const appInfo = getAppInfo();
</script>

<main class="flex h-full flex-col items-center justify-center gap-6 select-none">
  <h1 class="text-sm font-medium tracking-[0.4em] text-neutral-900">WHITE ROOM</h1>

  {#await appInfo}
    <p class="text-xs tracking-widest text-neutral-400">INITIALIZING</p>
  {:then info}
    <dl class="grid grid-cols-2 gap-x-6 gap-y-1 text-xs text-neutral-500">
      <dt class="text-right tracking-widest">VERSION</dt>
      <dd class="font-mono">{info.version}</dd>
      <dt class="text-right tracking-widest">SCHEMA</dt>
      <dd class="font-mono">{info.schemaVersion}</dd>
    </dl>
  {:catch error}
    <p class="max-w-md text-center text-xs text-neutral-500" role="alert">
      Backend unavailable: {String(error)}
    </p>
  {/await}
</main>
