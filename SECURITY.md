# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.2.x   | :white_check_mark: |
| < 0.2   | :x:                |

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability in ALEC, please report it responsibly.

### How to Report

**DO NOT** open a public GitHub issue for security vulnerabilities.

Instead, please email: **security@alec-codec.com**

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### What to Expect

- **Acknowledgment**: Within 48 hours
- **Initial Assessment**: Within 7 days
- **Resolution Timeline**: Depends on severity
  - Critical: 24-48 hours
  - High: 7 days
  - Medium: 30 days
  - Low: 90 days

### Disclosure Policy

- We will work with you to understand and resolve the issue
- We will credit you in the security advisory (unless you prefer anonymity)
- We ask that you do not disclose the vulnerability publicly until we have released a fix

## Security Best Practices for ALEC Users

1. **Keep ALEC updated** to the latest version
2. **Use TLS/DTLS** for network communication
3. **Validate preload files** from untrusted sources
4. **Enable rate limiting** in production deployments
5. **Review audit logs** regularly

## Known Security Considerations

- Preload files should only be loaded from trusted sources
- Context synchronization should occur over secure channels
- Rate limiting should be enabled to prevent DoS attacks

Thank you for helping keep ALEC secure!
