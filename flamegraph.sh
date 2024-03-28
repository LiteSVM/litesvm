# See this if the last part (where it says "perf record")
# is slow https://github.com/flamegraph-rs/flamegraph/issues/74#issuecomment-1909417039
CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph --bench max_perf -- --bench
