// Class name utilities
export { cn } from './cn';

// Formatting utilities
export {
	formatBytes,
	formatDuration,
	formatDate,
	formatDatetime,
	formatPercent,
	formatNumber,
	formatEta,
	formatPlatformName
} from './format';

// Time utilities
export {
	parseISODate,
	calculateTokenExpiry,
	getTokenTimeRemaining,
	isTokenExpired,
	isTokenExpiringSoon,
	addDays,
	getRelativeTime
} from './time';

// Error handling utilities
export {
	extractErrorMessage,
	createErrorHandler,
	isApiError,
	getApiErrorCode,
	isNetworkError,
	isAuthError
} from './errors';

// API utilities
export {
	transformChannel,
	normalizeUrl,
	convertToWsUrl,
	unwrapApiResponse,
	buildQueryString,
	type BackendChannel
} from './api';

// Constants
export {
	RECONNECT_DELAYS,
	MAX_RECONNECT_TIME,
	MAX_ACTIVITY_EVENTS,
	TOKEN_EXPIRY_WARNING_DAYS,
	TOKEN_DEFAULT_LIFETIME_DAYS,
	PLATFORMS,
	QUALITY_OPTIONS,
	TIMEZONE_OPTIONS,
	OUTPUT_FORMATS,
	RECORDING_STATUS_COLORS,
	CHANNEL_STATUS_COLORS,
	RECORDING_STATUS_TEXT_COLORS,
	CRF_RANGE,
	ENCODING_PRESETS,
	VIDEO_CODECS,
	AUDIO_CODECS,
	DAYS_OF_WEEK,
	DEFAULT_PAGE_SIZE,
	PAGE_SIZE_OPTIONS,
	ANIMATION_DURATION,
	POLLING_INTERVALS,
	DOWNLOAD_STATUS_COLORS,
	DOWNLOAD_STATUS_LABELS,
	type Platform
} from './constants';
