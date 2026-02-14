import { describe, it, expect } from 'vitest';
import { isNewerVersion, isCompatibleMin, isCompatibleMax } from './version';

describe('isNewerVersion', () => {
	it('detects newer patch version', () => {
		expect(isNewerVersion('1.0.0', '1.0.1')).toBe(true);
	});

	it('detects newer minor version', () => {
		expect(isNewerVersion('1.0.0', '1.1.0')).toBe(true);
	});

	it('detects newer major version', () => {
		expect(isNewerVersion('1.0.0', '2.0.0')).toBe(true);
	});

	it('returns false for same version', () => {
		expect(isNewerVersion('1.0.0', '1.0.0')).toBe(false);
		expect(isNewerVersion('2.5.3', '2.5.3')).toBe(false);
	});

	it('returns false for older version', () => {
		expect(isNewerVersion('1.0.1', '1.0.0')).toBe(false);
		expect(isNewerVersion('2.0.0', '1.9.9')).toBe(false);
	});

	it('handles v prefix', () => {
		expect(isNewerVersion('v1.0.0', 'v1.0.1')).toBe(true);
		expect(isNewerVersion('1.0.0', 'v2.0.0')).toBe(true);
		expect(isNewerVersion('v1.0.1', '1.0.0')).toBe(false);
	});

	it('handles partial versions', () => {
		expect(isNewerVersion('1', '2')).toBe(true);
		expect(isNewerVersion('1.0', '1.1')).toBe(true);
		expect(isNewerVersion('2', '1')).toBe(false);
	});
});

describe('isCompatibleMin', () => {
	it('returns true when client equals minimum', () => {
		expect(isCompatibleMin('1.0.0', '1.0.0')).toBe(true);
	});

	it('returns true when client is newer than minimum', () => {
		expect(isCompatibleMin('1.1.0', '1.0.0')).toBe(true);
		expect(isCompatibleMin('2.0.0', '1.0.0')).toBe(true);
	});

	it('returns false when client is older than minimum', () => {
		expect(isCompatibleMin('0.9.0', '1.0.0')).toBe(false);
		expect(isCompatibleMin('1.0.0', '1.0.1')).toBe(false);
	});
});

describe('isCompatibleMax', () => {
	it('returns true when client equals maximum', () => {
		expect(isCompatibleMax('1.99.99', '1.99.99')).toBe(true);
	});

	it('returns true when client is older than maximum', () => {
		expect(isCompatibleMax('1.0.0', '1.99.99')).toBe(true);
	});

	it('returns false when client is newer than maximum', () => {
		expect(isCompatibleMax('2.0.0', '1.99.99')).toBe(false);
	});
});
