<script lang="ts">
	import { Upload, Trash2, ImageIcon, AlertCircle, Download } from 'lucide-svelte';
	import type { ChannelProfile } from '$lib/api/types';
	import { api } from '$lib/api';
	import { toastStore } from '$lib/stores/toast.svelte';
	import { extractErrorMessage } from '$lib/utils';
	import ChannelAvatar from './ChannelAvatar.svelte';
	import Card from './ui/Card.svelte';

	interface Props {
		channelId: string;
		profile: ChannelProfile;
		onProfileUpdate?: () => void;
	}

	let { channelId, profile, onProfileUpdate }: Props = $props();

	// Loading states
	let uploadingProfile = $state(false);
	let uploadingBanner = $state(false);
	let deletingProfile = $state(false);
	let deletingBanner = $state(false);
	let fetchingPlatform = $state(false);

	// File input references
	let profileInput: HTMLInputElement | null = $state(null);
	let bannerInput: HTMLInputElement | null = $state(null);

	// Derived image URLs - prefer custom over platform
	const profileUrl = $derived(profile.custom_profile_url ?? profile.platform_profile_url);
	const bannerUrl = $derived(profile.custom_banner_url ?? profile.platform_banner_url);
	const hasCustomProfile = $derived(!!profile.custom_profile_url);
	const hasCustomBanner = $derived(!!profile.custom_banner_url);

	async function handleProfileUpload(event: Event) {
		const input = event.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		uploadingProfile = true;
		try {
			await api.uploadChannelImage(channelId, 'profile', file);
			toastStore.success('Profile image uploaded');
			onProfileUpdate?.();
		} catch (e) {
			toastStore.error(extractErrorMessage(e, 'Failed to upload profile image'));
		} finally {
			uploadingProfile = false;
			// Reset input
			if (input) input.value = '';
		}
	}

	async function handleBannerUpload(event: Event) {
		const input = event.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		uploadingBanner = true;
		try {
			await api.uploadChannelImage(channelId, 'banner', file);
			toastStore.success('Banner image uploaded');
			onProfileUpdate?.();
		} catch (e) {
			toastStore.error(extractErrorMessage(e, 'Failed to upload banner image'));
		} finally {
			uploadingBanner = false;
			// Reset input
			if (input) input.value = '';
		}
	}

	async function handleDeleteProfile() {
		if (!hasCustomProfile) return;

		deletingProfile = true;
		try {
			await api.deleteChannelImage(channelId, 'profile');
			toastStore.success('Profile image removed');
			onProfileUpdate?.();
		} catch (e) {
			toastStore.error(extractErrorMessage(e, 'Failed to remove profile image'));
		} finally {
			deletingProfile = false;
		}
	}

	async function handleDeleteBanner() {
		if (!hasCustomBanner) return;

		deletingBanner = true;
		try {
			await api.deleteChannelImage(channelId, 'banner');
			toastStore.success('Banner image removed');
			onProfileUpdate?.();
		} catch (e) {
			toastStore.error(extractErrorMessage(e, 'Failed to remove banner image'));
		} finally {
			deletingBanner = false;
		}
	}

	async function handleFetchPlatformImages() {
		fetchingPlatform = true;
		try {
			const result = await api.fetchPlatformImages(channelId);
			if (result.success) {
				if (result.profile_image_url || result.banner_image_url) {
					toastStore.success('Platform images fetched');
				} else {
					toastStore.info('No platform images available');
				}
				onProfileUpdate?.();
			}
		} catch (e) {
			toastStore.error(extractErrorMessage(e, 'Failed to fetch platform images'));
		} finally {
			fetchingPlatform = false;
		}
	}
</script>

<div class="space-y-6">
	<!-- Fetch from Platform Action -->
	<Card padding="md">
		<div class="flex items-center justify-between">
			<div>
				<h4 class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-1">
					Platform Images
				</h4>
				<p class="text-xs text-zinc-400">
					{#if profile.platform_profile_url || profile.platform_banner_url}
						Refresh profile and banner images from {profile.platform}
					{:else}
						Fetch profile and banner images from {profile.platform}
					{/if}
				</p>
			</div>
			<button
				class="flex items-center gap-1.5 px-3 py-1.5 bg-emerald-500/10 border border-emerald-500/30 rounded text-xs font-mono text-emerald-400 hover:bg-emerald-500/20 transition-colors disabled:opacity-50"
				onclick={handleFetchPlatformImages}
				disabled={fetchingPlatform}
			>
				{#if fetchingPlatform}
					<div class="size-3 animate-spin rounded-full border-2 border-emerald-400 border-t-transparent"></div>
				{:else}
					<Download class="size-3" />
				{/if}
				{profile.platform_profile_url || profile.platform_banner_url ? 'Refresh' : 'Fetch'}
			</button>
		</div>
	</Card>

	<!-- Profile Image Section -->
	<Card padding="md">
		<div class="flex items-start gap-4">
			<ChannelAvatar
				src={profileUrl}
				alt={profile.display_name}
				platform={profile.platform}
				size="xl"
				showBadge={false}
			/>
			<div class="flex-1">
				<h4 class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-2">
					Profile Image
				</h4>
				<p class="text-xs text-zinc-400 mb-3">
					{#if hasCustomProfile}
						Using custom image
					{:else if profile.platform_profile_url}
						Using {profile.platform} profile image
					{:else}
						No profile image
					{/if}
				</p>
				<div class="flex gap-2">
					<input
						type="file"
						accept="image/jpeg,image/png,image/webp,image/gif"
						class="hidden"
						bind:this={profileInput}
						onchange={handleProfileUpload}
					/>
					<button
						class="flex items-center gap-1.5 px-3 py-1.5 bg-zinc-800 border border-zinc-700 rounded text-xs font-mono text-zinc-300 hover:bg-zinc-700 transition-colors disabled:opacity-50"
						onclick={() => profileInput?.click()}
						disabled={uploadingProfile}
					>
						{#if uploadingProfile}
							<div class="size-3 animate-spin rounded-full border-2 border-zinc-400 border-t-transparent"></div>
						{:else}
							<Upload class="size-3" />
						{/if}
						Upload
					</button>
					{#if hasCustomProfile}
						<button
							class="flex items-center gap-1.5 px-3 py-1.5 bg-red-500/10 border border-red-500/30 rounded text-xs font-mono text-red-400 hover:bg-red-500/20 transition-colors disabled:opacity-50"
							onclick={handleDeleteProfile}
							disabled={deletingProfile}
						>
							{#if deletingProfile}
								<div class="size-3 animate-spin rounded-full border-2 border-red-400 border-t-transparent"></div>
							{:else}
								<Trash2 class="size-3" />
							{/if}
							Remove
						</button>
					{/if}
				</div>
				<p class="text-[10px] text-zinc-500 mt-2">
					Max 5MB. Will be resized to 300×300.
				</p>
			</div>
		</div>
	</Card>

	<!-- Banner Image Section -->
	<Card padding="md">
		<div>
			<h4 class="font-mono text-[10px] uppercase tracking-wider text-zinc-500 mb-2">
				Banner Image
			</h4>

			<!-- Banner Preview -->
			<div class="relative w-full h-32 bg-zinc-800 rounded border border-zinc-700 mb-3 overflow-hidden">
				{#if bannerUrl}
					<img
						src={bannerUrl}
						alt="{profile.display_name} banner"
						class="w-full h-full object-cover"
					/>
				{:else}
					<div class="flex items-center justify-center h-full text-zinc-600">
						<ImageIcon class="size-8" />
					</div>
				{/if}
			</div>

			<p class="text-xs text-zinc-400 mb-3">
				{#if hasCustomBanner}
					Using custom banner
				{:else if profile.platform_banner_url}
					Using {profile.platform} banner
				{:else}
					No banner image
				{/if}
			</p>

			<div class="flex gap-2">
				<input
					type="file"
					accept="image/jpeg,image/png,image/webp,image/gif"
					class="hidden"
					bind:this={bannerInput}
					onchange={handleBannerUpload}
				/>
				<button
					class="flex items-center gap-1.5 px-3 py-1.5 bg-zinc-800 border border-zinc-700 rounded text-xs font-mono text-zinc-300 hover:bg-zinc-700 transition-colors disabled:opacity-50"
					onclick={() => bannerInput?.click()}
					disabled={uploadingBanner}
				>
					{#if uploadingBanner}
						<div class="size-3 animate-spin rounded-full border-2 border-zinc-400 border-t-transparent"></div>
					{:else}
						<Upload class="size-3" />
					{/if}
					Upload
				</button>
				{#if hasCustomBanner}
					<button
						class="flex items-center gap-1.5 px-3 py-1.5 bg-red-500/10 border border-red-500/30 rounded text-xs font-mono text-red-400 hover:bg-red-500/20 transition-colors disabled:opacity-50"
						onclick={handleDeleteBanner}
						disabled={deletingBanner}
					>
						{#if deletingBanner}
							<div class="size-3 animate-spin rounded-full border-2 border-red-400 border-t-transparent"></div>
						{:else}
							<Trash2 class="size-3" />
						{/if}
						Remove
					</button>
				{/if}
			</div>
			<p class="text-[10px] text-zinc-500 mt-2">
				Max 10MB. Will be resized to 1200×400.
			</p>
		</div>
	</Card>
</div>
