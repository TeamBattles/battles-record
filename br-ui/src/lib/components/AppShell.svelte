<script lang="ts">
	import Sidebar from './Sidebar.svelte';
	import Titlebar from './Titlebar.svelte';
	import StatusBar from './StatusBar.svelte';
	import ReconnectBanner from './ReconnectBanner.svelte';

	interface Props {
		children: import('svelte').Snippet;
		onAddServer: () => void;
		onManageServers: () => void;
	}

	let { children, onAddServer, onManageServers }: Props = $props();
</script>

<div class="h-screen flex flex-col bg-background-deep">
	<Titlebar {onAddServer} {onManageServers} />
	<ReconnectBanner />
	<div class="flex flex-1 overflow-hidden">
		<Sidebar />
		<main class="flex-1 overflow-auto p-4 lg:p-6 relative">
			{@render children()}
		</main>
	</div>
	<StatusBar />
</div>
