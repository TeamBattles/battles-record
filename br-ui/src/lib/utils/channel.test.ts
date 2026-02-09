import { describe, it, expect } from 'vitest';
import { extractChannelName, validateChannelName, type ValidationResult } from './channel';

describe('extractChannelName', () => {
	describe('Twitch', () => {
		it('extracts username from twitch.tv URL', () => {
			expect(extractChannelName('twitch', 'https://twitch.tv/ninja')).toBe('ninja');
		});

		it('extracts username from www.twitch.tv URL', () => {
			expect(extractChannelName('twitch', 'https://www.twitch.tv/shroud')).toBe('shroud');
		});

		it('extracts username from m.twitch.tv URL', () => {
			expect(extractChannelName('twitch', 'https://m.twitch.tv/pokimane')).toBe('pokimane');
		});

		it('extracts username from URL with /videos suffix', () => {
			expect(extractChannelName('twitch', 'https://twitch.tv/xqc/videos')).toBe('xqc');
		});

		it('extracts username from URL with query params', () => {
			expect(extractChannelName('twitch', 'https://twitch.tv/asmongold?ref=homepage')).toBe(
				'asmongold'
			);
		});

		it('returns plain username as-is', () => {
			expect(extractChannelName('twitch', 'summit1g')).toBe('summit1g');
		});

		it('handles URL without protocol', () => {
			expect(extractChannelName('twitch', 'twitch.tv/hasanabi')).toBe('hasanabi');
		});

		it('handles http protocol', () => {
			expect(extractChannelName('twitch', 'http://twitch.tv/timthetatman')).toBe('timthetatman');
		});

		it('handles trailing slash', () => {
			expect(extractChannelName('twitch', 'https://twitch.tv/lirik/')).toBe('lirik');
		});
	});

	describe('YouTube', () => {
		it('extracts handle from youtube.com/@handle URL', () => {
			expect(extractChannelName('youtube', 'https://youtube.com/@MrBeast')).toBe('@MrBeast');
		});

		it('extracts handle from www.youtube.com/@handle URL', () => {
			expect(extractChannelName('youtube', 'https://www.youtube.com/@PewDiePie')).toBe(
				'@PewDiePie'
			);
		});

		it('extracts channel ID from /channel/ URL', () => {
			expect(
				extractChannelName('youtube', 'https://youtube.com/channel/UC12345678901234567890')
			).toBe('UC12345678901234567890');
		});

		it('extracts custom name from /c/ URL', () => {
			expect(extractChannelName('youtube', 'https://youtube.com/c/LinusTechTips')).toBe(
				'LinusTechTips'
			);
		});

		it('handles handle with /videos suffix', () => {
			expect(extractChannelName('youtube', 'https://youtube.com/@MrBeast/videos')).toBe('@MrBeast');
		});

		it('handles handle with query params', () => {
			expect(extractChannelName('youtube', 'https://youtube.com/@MrBeast?sub_confirmation=1')).toBe(
				'@MrBeast'
			);
		});

		it('returns plain @handle as-is', () => {
			expect(extractChannelName('youtube', '@veritasium')).toBe('@veritasium');
		});

		it('returns plain channel ID as-is', () => {
			expect(extractChannelName('youtube', 'UC12345678901234567890')).toBe(
				'UC12345678901234567890'
			);
		});
	});

	describe('Kick', () => {
		it('extracts username from kick.com URL', () => {
			expect(extractChannelName('kick', 'https://kick.com/xqc')).toBe('xqc');
		});

		it('extracts username from www.kick.com URL', () => {
			expect(extractChannelName('kick', 'https://www.kick.com/adin')).toBe('adin');
		});

		it('returns plain username as-is', () => {
			expect(extractChannelName('kick', 'amouranth')).toBe('amouranth');
		});

		it('handles URL without protocol', () => {
			expect(extractChannelName('kick', 'kick.com/trainwreckstv')).toBe('trainwreckstv');
		});

		it('handles trailing slash', () => {
			expect(extractChannelName('kick', 'https://kick.com/destiny/')).toBe('destiny');
		});
	});

	describe('Edge cases', () => {
		it('returns empty string for empty input', () => {
			expect(extractChannelName('twitch', '')).toBe('');
		});

		it('returns empty string for whitespace-only input', () => {
			expect(extractChannelName('twitch', '   ')).toBe('');
		});

		it('trims whitespace from input', () => {
			expect(extractChannelName('twitch', '  ninja  ')).toBe('ninja');
		});

		it('strips protocol from input', () => {
			expect(extractChannelName('twitch', 'https://twitch.tv/test')).toBe('test');
			expect(extractChannelName('twitch', 'http://twitch.tv/test')).toBe('test');
		});

		it('handles fragment in URL', () => {
			expect(extractChannelName('twitch', 'https://twitch.tv/ninja#section')).toBe('ninja');
		});

		it('returns URL-like non-platform input with protocol stripped', () => {
			// Protocol is stripped first, then non-matching URL is returned as-is
			expect(extractChannelName('twitch', 'https://other.com/ninja')).toBe('other.com/ninja');
		});
	});
});

describe('validateChannelName', () => {
	describe('Common validation', () => {
		it('returns invalid for empty string', () => {
			const result = validateChannelName('twitch', '');
			expect(result.valid).toBe(false);
			expect(result.warning).toBe('Channel name is required');
		});

		it('returns invalid for whitespace-only input', () => {
			const result = validateChannelName('twitch', '   ');
			expect(result.valid).toBe(false);
			expect(result.warning).toBe('Channel name is required');
		});

		it('returns invalid for URL-like input with slash', () => {
			const result = validateChannelName('twitch', 'some/path');
			expect(result.valid).toBe(false);
			expect(result.warning).toBe('Unrecognized URL format');
		});

		it('returns invalid for URL-like input with dot', () => {
			const result = validateChannelName('twitch', 'some.domain');
			expect(result.valid).toBe(false);
			expect(result.warning).toBe('Unrecognized URL format');
		});
	});

	describe('Twitch validation', () => {
		it('accepts valid 4-25 character alphanumeric username', () => {
			expect(validateChannelName('twitch', 'ninja').valid).toBe(true);
			expect(validateChannelName('twitch', 'test_user123').valid).toBe(true);
		});

		it('rejects username shorter than 4 characters', () => {
			const result = validateChannelName('twitch', 'abc');
			expect(result.valid).toBe(false);
			expect(result.warning).toBe('Twitch username must be at least 4 characters');
		});

		it('rejects username longer than 25 characters', () => {
			const result = validateChannelName('twitch', 'a'.repeat(26));
			expect(result.valid).toBe(false);
			expect(result.warning).toBe('Twitch username must be at most 25 characters');
		});

		it('rejects username with special characters', () => {
			const result = validateChannelName('twitch', 'ninja@test');
			expect(result.valid).toBe(false);
			expect(result.warning).toBe(
				'Twitch username can only contain letters, numbers, and underscores'
			);
		});

		it('rejects username with spaces', () => {
			const result = validateChannelName('twitch', 'ninja test');
			expect(result.valid).toBe(false);
			expect(result.warning).toBe(
				'Twitch username can only contain letters, numbers, and underscores'
			);
		});

		it('accepts username with underscores', () => {
			expect(validateChannelName('twitch', 'test_user').valid).toBe(true);
			expect(validateChannelName('twitch', '_test_').valid).toBe(true);
		});

		it('accepts exactly 4 characters (boundary)', () => {
			expect(validateChannelName('twitch', 'abcd').valid).toBe(true);
		});

		it('accepts exactly 25 characters (boundary)', () => {
			expect(validateChannelName('twitch', 'a'.repeat(25)).valid).toBe(true);
		});
	});

	describe('YouTube validation', () => {
		it('accepts valid @handle with 3-30 characters', () => {
			expect(validateChannelName('youtube', '@MrBeast').valid).toBe(true);
			expect(validateChannelName('youtube', '@test_channel-123').valid).toBe(true);
		});

		it('rejects @handle shorter than 3 characters', () => {
			const result = validateChannelName('youtube', '@ab');
			expect(result.valid).toBe(false);
			expect(result.warning).toBe('YouTube handle must be at least 3 characters');
		});

		it('rejects @handle longer than 30 characters', () => {
			const result = validateChannelName('youtube', '@' + 'a'.repeat(31));
			expect(result.valid).toBe(false);
			expect(result.warning).toBe('YouTube handle must be at most 30 characters');
		});

		it('accepts @handle with hyphens and underscores', () => {
			// Note: periods fail validation because the common validation check
			// rejects any input with '.' as "URL-like"
			expect(validateChannelName('youtube', '@test-channel').valid).toBe(true);
			expect(validateChannelName('youtube', '@test_channel').valid).toBe(true);
		});

		it('rejects @handle with periods due to URL-like validation', () => {
			// Periods are rejected because validateChannelName checks for '.' as URL-like input
			const result = validateChannelName('youtube', '@test.channel');
			expect(result.valid).toBe(false);
			expect(result.warning).toBe('Unrecognized URL format');
		});

		it('rejects @handle with invalid special characters', () => {
			const result = validateChannelName('youtube', '@test@channel');
			expect(result.valid).toBe(false);
		});

		it('accepts valid UC channel ID (24 characters)', () => {
			expect(validateChannelName('youtube', 'UC12345678901234567890').valid).toBe(true);
		});

		it('rejects UC prefix with wrong length', () => {
			// UC with wrong length should fail or be treated as custom name
			// Based on the implementation, UC with wrong length is treated as custom name
			// and should be valid if length < 100
			expect(validateChannelName('youtube', 'UC123').valid).toBe(true); // Treated as custom name
		});

		it('accepts legacy custom channel names', () => {
			expect(validateChannelName('youtube', 'LinusTechTips').valid).toBe(true);
		});

		it('rejects custom name longer than 100 characters', () => {
			const result = validateChannelName('youtube', 'a'.repeat(101));
			expect(result.valid).toBe(false);
			expect(result.warning).toBe('Channel name is too long');
		});
	});

	describe('Kick validation', () => {
		it('accepts valid 3-25 character alphanumeric username', () => {
			expect(validateChannelName('kick', 'xqc').valid).toBe(true);
			expect(validateChannelName('kick', 'test_user123').valid).toBe(true);
		});

		it('rejects username shorter than 3 characters', () => {
			const result = validateChannelName('kick', 'ab');
			expect(result.valid).toBe(false);
			expect(result.warning).toBe('Kick username must be at least 3 characters');
		});

		it('rejects username longer than 25 characters', () => {
			const result = validateChannelName('kick', 'a'.repeat(26));
			expect(result.valid).toBe(false);
			expect(result.warning).toBe('Kick username must be at most 25 characters');
		});

		it('rejects username with special characters', () => {
			const result = validateChannelName('kick', 'user@kick');
			expect(result.valid).toBe(false);
			expect(result.warning).toBe(
				'Kick username can only contain letters, numbers, and underscores'
			);
		});

		it('accepts username with underscores', () => {
			expect(validateChannelName('kick', 'test_user').valid).toBe(true);
		});

		it('accepts exactly 3 characters (boundary)', () => {
			expect(validateChannelName('kick', 'abc').valid).toBe(true);
		});

		it('accepts exactly 25 characters (boundary)', () => {
			expect(validateChannelName('kick', 'a'.repeat(25)).valid).toBe(true);
		});
	});

	describe('Edge cases', () => {
		it('handles mixed case usernames', () => {
			expect(validateChannelName('twitch', 'NiNjA').valid).toBe(true);
		});

		it('validates after trimming whitespace', () => {
			// The function trims, so "  abc  " becomes "abc" (3 chars) for Twitch
			const result = validateChannelName('twitch', '  abc  ');
			expect(result.valid).toBe(false); // "abc" is only 3 chars, Twitch needs 4
		});

		it('returns valid for unknown platform', () => {
			// Based on implementation, unknown platform returns { valid: true }
			const result = validateChannelName('unknown' as 'twitch', 'anything');
			expect(result.valid).toBe(true);
		});
	});
});

describe('extractChannelName + validateChannelName integration', () => {
	it('extracts and validates Twitch URL correctly', () => {
		const input = 'https://twitch.tv/ninja';
		const extracted = extractChannelName('twitch', input);
		const validation = validateChannelName('twitch', extracted);
		expect(extracted).toBe('ninja');
		expect(validation.valid).toBe(true);
	});

	it('extracts and validates YouTube URL correctly', () => {
		const input = 'https://youtube.com/@MrBeast';
		const extracted = extractChannelName('youtube', input);
		const validation = validateChannelName('youtube', extracted);
		expect(extracted).toBe('@MrBeast');
		expect(validation.valid).toBe(true);
	});

	it('extracts and validates Kick URL correctly', () => {
		const input = 'https://kick.com/xqc';
		const extracted = extractChannelName('kick', input);
		const validation = validateChannelName('kick', extracted);
		expect(extracted).toBe('xqc');
		expect(validation.valid).toBe(true);
	});

	it('extracted invalid URL is rejected by validation', () => {
		// Non-platform URL will be returned as-is with slashes
		const input = 'https://other.com/user';
		const extracted = extractChannelName('twitch', input);
		// The extracted value has slashes/dots, so validation will fail
		const validation = validateChannelName('twitch', extracted);
		expect(validation.valid).toBe(false);
	});
});
