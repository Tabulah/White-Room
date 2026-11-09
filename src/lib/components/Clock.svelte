<!-- Local time and date, in the user's locale. -->
<script lang="ts">
  let now = $state(new Date());

  $effect(() => {
    const timer = setInterval(() => (now = new Date()), 1000);
    return () => clearInterval(timer);
  });

  const time = $derived(now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }));
  const date = $derived(
    now.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' }),
  );
</script>

<div class="flex flex-col items-center gap-4">
  <time
    class="text-display font-light tracking-tight text-ink tabular-nums"
    datetime={now.toISOString()}
  >
    {time}
  </time>
  <p class="text-micro tracking-label text-ink-faint uppercase">{date}</p>
</div>
