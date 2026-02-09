import type { Channel, Recording } from '$lib/api/types';

export const mockChannels: Channel[] = [
	{
		id: 'ch-1',
		name: 'test_streamer',
		platform: 'twitch',
		enabled: true,
		quality: 'best',
		status: {
			is_live: false,
			is_recording: false
		}
	},
	{
		id: 'ch-2',
		name: '@youtube_channel',
		platform: 'youtube',
		enabled: true,
		quality: '1080p',
		status: {
			is_live: true,
			is_recording: false,
			current_stream: {
				title: 'Live Stream Title',
				game: 'Just Chatting',
				viewer_count: 1000,
				started_at: '2024-01-15T10:00:00Z'
			}
		}
	},
	{
		id: 'ch-3',
		name: 'kick_streamer',
		platform: 'kick',
		enabled: false,
		quality: '720p',
		status: {
			is_live: true,
			is_recording: true,
			current_stream: {
				title: 'Gaming Stream',
				game: 'Valorant',
				viewer_count: 500,
				started_at: '2024-01-15T09:00:00Z'
			}
		},
		quota_gb: 10,
		quota_status: 'warning',
		quota_used_bytes: 8_500_000_000,
		quota_percent: 85
	}
];

export const mockRecordings: Recording[] = [
	{
		id: 'rec-1',
		channel_name: 'test_streamer',
		platform: 'twitch',
		started_at: '2024-01-15T10:00:00Z',
		ended_at: '2024-01-15T12:00:00Z',
		duration_secs: 7200,
		status: 'processed',
		path: '/recordings/twitch/test_streamer/20240115_100000',
		size_bytes: 1_500_000_000,
		title: 'Morning Stream',
		game: 'Minecraft',
		output_file: '/library/twitch/test_streamer/20240115_100000.mp4'
	},
	{
		id: 'rec-2',
		channel_name: 'test_streamer',
		platform: 'twitch',
		started_at: '2024-01-14T18:00:00Z',
		ended_at: '2024-01-14T20:00:00Z',
		duration_secs: 7200,
		status: 'pending_processing',
		path: '/recordings/twitch/test_streamer/20240114_180000',
		size_bytes: 2_000_000_000,
		title: 'Evening Stream',
		game: 'Fortnite'
	},
	{
		id: 'rec-3',
		channel_name: 'kick_streamer',
		platform: 'kick',
		started_at: '2024-01-15T09:00:00Z',
		status: 'recording',
		path: '/recordings/kick/kick_streamer/20240115_090000',
		size_bytes: 500_000_000,
		title: 'Gaming Stream',
		game: 'Valorant'
	}
];
