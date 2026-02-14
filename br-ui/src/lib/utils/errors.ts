/**
 * Extract error message from unknown error type
 */
export function extractErrorMessage(
	error: unknown,
	fallback: string = 'An unexpected error occurred'
): string {
	if (error instanceof Error) {
		return error.message;
	}
	if (typeof error === 'string') {
		return error;
	}
	if (error && typeof error === 'object' && 'message' in error) {
		return String((error as { message: unknown }).message);
	}
	return fallback;
}

/**
 * Create an error handler function for a specific action
 */
export function createErrorHandler(action: string): (error: unknown) => string {
	return (error: unknown) => extractErrorMessage(error, `Failed to ${action}`);
}

/**
 * Type guard for API errors
 */
export function isApiError(error: unknown): error is { status: number; message: string } {
	return (
		typeof error === 'object' &&
		error !== null &&
		'status' in error &&
		'message' in error &&
		typeof (error as { status: unknown }).status === 'number' &&
		typeof (error as { message: unknown }).message === 'string'
	);
}

/**
 * Get API error code if available
 */
export function getApiErrorCode(error: unknown): number | null {
	if (isApiError(error)) {
		return error.status;
	}
	return null;
}

/**
 * Check if error is a network error
 */
export function isNetworkError(error: unknown): boolean {
	if (error instanceof Error) {
		const message = error.message.toLowerCase();
		return (
			message.includes('network') ||
			message.includes('fetch') ||
			message.includes('connection') ||
			message.includes('timeout')
		);
	}
	return false;
}

/**
 * Check if error is an authentication error
 */
export function isAuthError(error: unknown): boolean {
	if (isApiError(error)) {
		return error.status === 401 || error.status === 403;
	}
	return false;
}
