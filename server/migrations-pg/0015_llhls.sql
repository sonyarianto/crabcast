-- Phase 11 LL-HLS increment: 2s segments cut live latency roughly in half
-- (players wait for ~3 segments instead of buffering 15+s of 5s chunks).
UPDATE stations SET hls_segment_seconds = 2.0 WHERE hls_segment_seconds = 5.0;
