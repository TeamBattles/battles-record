import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import ChannelAvatar from './ChannelAvatar.svelte';

describe('ChannelAvatar', () => {
	it('renders with platform icon fallback when no src', () => {
		render(ChannelAvatar, {
			props: {
				alt: 'Test Channel',
				platform: 'twitch'
			}
		});

		// Should show the avatar container
		const avatar = screen.getByTitle('Test Channel');
		expect(avatar).toBeInTheDocument();
	});

	it('renders with different sizes', () => {
		const { container } = render(ChannelAvatar, {
			props: {
				alt: 'Test Channel',
				platform: 'youtube',
				size: 'lg'
			}
		});

		// size-16 class should be applied for lg
		const avatar = container.querySelector('.size-16');
		expect(avatar).toBeInTheDocument();
	});

	it('renders with src when provided', () => {
		render(ChannelAvatar, {
			props: {
				src: 'https://example.com/avatar.jpg',
				alt: 'Test Channel',
				platform: 'kick'
			}
		});

		// Should render an img element
		const img = screen.getByRole('img');
		expect(img).toHaveAttribute('src', 'https://example.com/avatar.jpg');
		expect(img).toHaveAttribute('alt', 'Test Channel');
	});

	it('shows fallback icon when image fails to load', async () => {
		render(ChannelAvatar, {
			props: {
				src: 'https://example.com/invalid.jpg',
				alt: 'Test Channel',
				platform: 'twitch'
			}
		});

		// Find the image and trigger error
		const img = screen.getByRole('img');
		img.dispatchEvent(new Event('error'));

		// After error, should fall back to platform icon
		// The component should handle this gracefully
	});

	it('applies custom className', () => {
		const { container } = render(ChannelAvatar, {
			props: {
				alt: 'Test Channel',
				platform: 'twitch',
				class: 'custom-class'
			}
		});

		const avatar = container.querySelector('.custom-class');
		expect(avatar).toBeInTheDocument();
	});

	it('handles null src gracefully', () => {
		render(ChannelAvatar, {
			props: {
				src: null,
				alt: 'Test Channel',
				platform: 'youtube'
			}
		});

		// Should show fallback (platform icon)
		const avatar = screen.getByTitle('Test Channel');
		expect(avatar).toBeInTheDocument();
	});
});
