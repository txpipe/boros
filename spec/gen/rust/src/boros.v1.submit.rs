// @generated
// This file is @generated by prost-build.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Tx {
    /// Raw transaction data.
    #[prost(bytes="bytes", tag="1")]
    pub raw: ::prost::bytes::Bytes,
    #[prost(string, optional, tag="2")]
    pub queue: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag="3")]
    pub lock_token: ::core::option::Option<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubmitTxRequest {
    #[prost(message, repeated, tag="1")]
    pub tx: ::prost::alloc::vec::Vec<Tx>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubmitTxResponse {
    #[prost(bytes="bytes", repeated, tag="1")]
    pub r#ref: ::prost::alloc::vec::Vec<::prost::bytes::Bytes>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LockStateRequest {
    #[prost(string, tag="1")]
    pub queue: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LockStateResponse {
    #[prost(string, tag="1")]
    pub lock_token: ::prost::alloc::string::String,
    /// TODO: use u5c transaction
    #[prost(string, tag="2")]
    pub cbor: ::prost::alloc::string::String,
}
/// Encoded file descriptor set for the `boros.v1.submit` package
pub const FILE_DESCRIPTOR_SET: &[u8] = &[
    0x0a, 0x98, 0x0c, 0x0a, 0x15, 0x62, 0x6f, 0x72, 0x6f, 0x73, 0x2f, 0x76, 0x31, 0x2f, 0x73, 0x75,
    0x62, 0x6d, 0x69, 0x74, 0x2e, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x12, 0x0f, 0x62, 0x6f, 0x72, 0x6f,
    0x73, 0x2e, 0x76, 0x31, 0x2e, 0x73, 0x75, 0x62, 0x6d, 0x69, 0x74, 0x22, 0x6e, 0x0a, 0x02, 0x54,
    0x78, 0x12, 0x10, 0x0a, 0x03, 0x72, 0x61, 0x77, 0x18, 0x01, 0x20, 0x01, 0x28, 0x0c, 0x52, 0x03,
    0x72, 0x61, 0x77, 0x12, 0x19, 0x0a, 0x05, 0x71, 0x75, 0x65, 0x75, 0x65, 0x18, 0x02, 0x20, 0x01,
    0x28, 0x09, 0x48, 0x00, 0x52, 0x05, 0x71, 0x75, 0x65, 0x75, 0x65, 0x88, 0x01, 0x01, 0x12, 0x22,
    0x0a, 0x0a, 0x6c, 0x6f, 0x63, 0x6b, 0x5f, 0x74, 0x6f, 0x6b, 0x65, 0x6e, 0x18, 0x03, 0x20, 0x01,
    0x28, 0x09, 0x48, 0x01, 0x52, 0x09, 0x6c, 0x6f, 0x63, 0x6b, 0x54, 0x6f, 0x6b, 0x65, 0x6e, 0x88,
    0x01, 0x01, 0x42, 0x08, 0x0a, 0x06, 0x5f, 0x71, 0x75, 0x65, 0x75, 0x65, 0x42, 0x0d, 0x0a, 0x0b,
    0x5f, 0x6c, 0x6f, 0x63, 0x6b, 0x5f, 0x74, 0x6f, 0x6b, 0x65, 0x6e, 0x22, 0x36, 0x0a, 0x0f, 0x53,
    0x75, 0x62, 0x6d, 0x69, 0x74, 0x54, 0x78, 0x52, 0x65, 0x71, 0x75, 0x65, 0x73, 0x74, 0x12, 0x23,
    0x0a, 0x02, 0x74, 0x78, 0x18, 0x01, 0x20, 0x03, 0x28, 0x0b, 0x32, 0x13, 0x2e, 0x62, 0x6f, 0x72,
    0x6f, 0x73, 0x2e, 0x76, 0x31, 0x2e, 0x73, 0x75, 0x62, 0x6d, 0x69, 0x74, 0x2e, 0x54, 0x78, 0x52,
    0x02, 0x74, 0x78, 0x22, 0x24, 0x0a, 0x10, 0x53, 0x75, 0x62, 0x6d, 0x69, 0x74, 0x54, 0x78, 0x52,
    0x65, 0x73, 0x70, 0x6f, 0x6e, 0x73, 0x65, 0x12, 0x10, 0x0a, 0x03, 0x72, 0x65, 0x66, 0x18, 0x01,
    0x20, 0x03, 0x28, 0x0c, 0x52, 0x03, 0x72, 0x65, 0x66, 0x22, 0x28, 0x0a, 0x10, 0x4c, 0x6f, 0x63,
    0x6b, 0x53, 0x74, 0x61, 0x74, 0x65, 0x52, 0x65, 0x71, 0x75, 0x65, 0x73, 0x74, 0x12, 0x14, 0x0a,
    0x05, 0x71, 0x75, 0x65, 0x75, 0x65, 0x18, 0x01, 0x20, 0x01, 0x28, 0x09, 0x52, 0x05, 0x71, 0x75,
    0x65, 0x75, 0x65, 0x22, 0x46, 0x0a, 0x11, 0x4c, 0x6f, 0x63, 0x6b, 0x53, 0x74, 0x61, 0x74, 0x65,
    0x52, 0x65, 0x73, 0x70, 0x6f, 0x6e, 0x73, 0x65, 0x12, 0x1d, 0x0a, 0x0a, 0x6c, 0x6f, 0x63, 0x6b,
    0x5f, 0x74, 0x6f, 0x6b, 0x65, 0x6e, 0x18, 0x01, 0x20, 0x01, 0x28, 0x09, 0x52, 0x09, 0x6c, 0x6f,
    0x63, 0x6b, 0x54, 0x6f, 0x6b, 0x65, 0x6e, 0x12, 0x12, 0x0a, 0x04, 0x63, 0x62, 0x6f, 0x72, 0x18,
    0x02, 0x20, 0x01, 0x28, 0x09, 0x52, 0x04, 0x63, 0x62, 0x6f, 0x72, 0x32, 0xb4, 0x01, 0x0a, 0x0d,
    0x53, 0x75, 0x62, 0x6d, 0x69, 0x74, 0x53, 0x65, 0x72, 0x76, 0x69, 0x63, 0x65, 0x12, 0x4f, 0x0a,
    0x08, 0x53, 0x75, 0x62, 0x6d, 0x69, 0x74, 0x54, 0x78, 0x12, 0x20, 0x2e, 0x62, 0x6f, 0x72, 0x6f,
    0x73, 0x2e, 0x76, 0x31, 0x2e, 0x73, 0x75, 0x62, 0x6d, 0x69, 0x74, 0x2e, 0x53, 0x75, 0x62, 0x6d,
    0x69, 0x74, 0x54, 0x78, 0x52, 0x65, 0x71, 0x75, 0x65, 0x73, 0x74, 0x1a, 0x21, 0x2e, 0x62, 0x6f,
    0x72, 0x6f, 0x73, 0x2e, 0x76, 0x31, 0x2e, 0x73, 0x75, 0x62, 0x6d, 0x69, 0x74, 0x2e, 0x53, 0x75,
    0x62, 0x6d, 0x69, 0x74, 0x54, 0x78, 0x52, 0x65, 0x73, 0x70, 0x6f, 0x6e, 0x73, 0x65, 0x12, 0x52,
    0x0a, 0x09, 0x4c, 0x6f, 0x63, 0x6b, 0x53, 0x74, 0x61, 0x74, 0x65, 0x12, 0x21, 0x2e, 0x62, 0x6f,
    0x72, 0x6f, 0x73, 0x2e, 0x76, 0x31, 0x2e, 0x73, 0x75, 0x62, 0x6d, 0x69, 0x74, 0x2e, 0x4c, 0x6f,
    0x63, 0x6b, 0x53, 0x74, 0x61, 0x74, 0x65, 0x52, 0x65, 0x71, 0x75, 0x65, 0x73, 0x74, 0x1a, 0x22,
    0x2e, 0x62, 0x6f, 0x72, 0x6f, 0x73, 0x2e, 0x76, 0x31, 0x2e, 0x73, 0x75, 0x62, 0x6d, 0x69, 0x74,
    0x2e, 0x4c, 0x6f, 0x63, 0x6b, 0x53, 0x74, 0x61, 0x74, 0x65, 0x52, 0x65, 0x73, 0x70, 0x6f, 0x6e,
    0x73, 0x65, 0x42, 0xab, 0x01, 0x0a, 0x13, 0x63, 0x6f, 0x6d, 0x2e, 0x62, 0x6f, 0x72, 0x6f, 0x73,
    0x2e, 0x76, 0x31, 0x2e, 0x73, 0x75, 0x62, 0x6d, 0x69, 0x74, 0x42, 0x0b, 0x53, 0x75, 0x62, 0x6d,
    0x69, 0x74, 0x50, 0x72, 0x6f, 0x74, 0x6f, 0x50, 0x01, 0x5a, 0x29, 0x67, 0x69, 0x74, 0x68, 0x75,
    0x62, 0x2e, 0x63, 0x6f, 0x6d, 0x2f, 0x62, 0x75, 0x66, 0x62, 0x75, 0x69, 0x6c, 0x64, 0x2f, 0x62,
    0x75, 0x66, 0x2d, 0x74, 0x6f, 0x75, 0x72, 0x2f, 0x67, 0x65, 0x6e, 0x2f, 0x62, 0x6f, 0x72, 0x6f,
    0x73, 0x2f, 0x76, 0x31, 0xa2, 0x02, 0x03, 0x42, 0x56, 0x53, 0xaa, 0x02, 0x0f, 0x42, 0x6f, 0x72,
    0x6f, 0x73, 0x2e, 0x56, 0x31, 0x2e, 0x53, 0x75, 0x62, 0x6d, 0x69, 0x74, 0xca, 0x02, 0x0f, 0x42,
    0x6f, 0x72, 0x6f, 0x73, 0x5c, 0x56, 0x31, 0x5c, 0x53, 0x75, 0x62, 0x6d, 0x69, 0x74, 0xe2, 0x02,
    0x1b, 0x42, 0x6f, 0x72, 0x6f, 0x73, 0x5c, 0x56, 0x31, 0x5c, 0x53, 0x75, 0x62, 0x6d, 0x69, 0x74,
    0x5c, 0x47, 0x50, 0x42, 0x4d, 0x65, 0x74, 0x61, 0x64, 0x61, 0x74, 0x61, 0xea, 0x02, 0x11, 0x42,
    0x6f, 0x72, 0x6f, 0x73, 0x3a, 0x3a, 0x56, 0x31, 0x3a, 0x3a, 0x53, 0x75, 0x62, 0x6d, 0x69, 0x74,
    0x4a, 0xc0, 0x06, 0x0a, 0x06, 0x12, 0x04, 0x00, 0x00, 0x1b, 0x01, 0x0a, 0x08, 0x0a, 0x01, 0x0c,
    0x12, 0x03, 0x00, 0x00, 0x12, 0x0a, 0x08, 0x0a, 0x01, 0x02, 0x12, 0x03, 0x02, 0x00, 0x18, 0x0a,
    0x0a, 0x0a, 0x02, 0x04, 0x00, 0x12, 0x04, 0x04, 0x00, 0x08, 0x01, 0x0a, 0x0a, 0x0a, 0x03, 0x04,
    0x00, 0x01, 0x12, 0x03, 0x04, 0x08, 0x0a, 0x0a, 0x24, 0x0a, 0x04, 0x04, 0x00, 0x02, 0x00, 0x12,
    0x03, 0x05, 0x02, 0x10, 0x22, 0x17, 0x20, 0x52, 0x61, 0x77, 0x20, 0x74, 0x72, 0x61, 0x6e, 0x73,
    0x61, 0x63, 0x74, 0x69, 0x6f, 0x6e, 0x20, 0x64, 0x61, 0x74, 0x61, 0x2e, 0x0a, 0x0a, 0x0c, 0x0a,
    0x05, 0x04, 0x00, 0x02, 0x00, 0x05, 0x12, 0x03, 0x05, 0x02, 0x07, 0x0a, 0x0c, 0x0a, 0x05, 0x04,
    0x00, 0x02, 0x00, 0x01, 0x12, 0x03, 0x05, 0x08, 0x0b, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02,
    0x00, 0x03, 0x12, 0x03, 0x05, 0x0e, 0x0f, 0x0a, 0x0b, 0x0a, 0x04, 0x04, 0x00, 0x02, 0x01, 0x12,
    0x03, 0x06, 0x02, 0x1c, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x01, 0x04, 0x12, 0x03, 0x06,
    0x02, 0x0a, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x01, 0x05, 0x12, 0x03, 0x06, 0x0b, 0x11,
    0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x01, 0x01, 0x12, 0x03, 0x06, 0x12, 0x17, 0x0a, 0x0c,
    0x0a, 0x05, 0x04, 0x00, 0x02, 0x01, 0x03, 0x12, 0x03, 0x06, 0x1a, 0x1b, 0x0a, 0x0b, 0x0a, 0x04,
    0x04, 0x00, 0x02, 0x02, 0x12, 0x03, 0x07, 0x02, 0x21, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02,
    0x02, 0x04, 0x12, 0x03, 0x07, 0x02, 0x0a, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x02, 0x05,
    0x12, 0x03, 0x07, 0x0b, 0x11, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x02, 0x01, 0x12, 0x03,
    0x07, 0x12, 0x1c, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x00, 0x02, 0x02, 0x03, 0x12, 0x03, 0x07, 0x1f,
    0x20, 0x0a, 0x0a, 0x0a, 0x02, 0x04, 0x01, 0x12, 0x04, 0x09, 0x00, 0x0b, 0x01, 0x0a, 0x0a, 0x0a,
    0x03, 0x04, 0x01, 0x01, 0x12, 0x03, 0x09, 0x08, 0x17, 0x0a, 0x0b, 0x0a, 0x04, 0x04, 0x01, 0x02,
    0x00, 0x12, 0x03, 0x0a, 0x02, 0x15, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x01, 0x02, 0x00, 0x04, 0x12,
    0x03, 0x0a, 0x02, 0x0a, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x01, 0x02, 0x00, 0x06, 0x12, 0x03, 0x0a,
    0x0b, 0x0d, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x01, 0x02, 0x00, 0x01, 0x12, 0x03, 0x0a, 0x0e, 0x10,
    0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x01, 0x02, 0x00, 0x03, 0x12, 0x03, 0x0a, 0x13, 0x14, 0x0a, 0x0a,
    0x0a, 0x02, 0x04, 0x02, 0x12, 0x04, 0x0c, 0x00, 0x0e, 0x01, 0x0a, 0x0a, 0x0a, 0x03, 0x04, 0x02,
    0x01, 0x12, 0x03, 0x0c, 0x08, 0x18, 0x0a, 0x0b, 0x0a, 0x04, 0x04, 0x02, 0x02, 0x00, 0x12, 0x03,
    0x0d, 0x02, 0x19, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x02, 0x02, 0x00, 0x04, 0x12, 0x03, 0x0d, 0x02,
    0x0a, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x02, 0x02, 0x00, 0x05, 0x12, 0x03, 0x0d, 0x0b, 0x10, 0x0a,
    0x0c, 0x0a, 0x05, 0x04, 0x02, 0x02, 0x00, 0x01, 0x12, 0x03, 0x0d, 0x11, 0x14, 0x0a, 0x0c, 0x0a,
    0x05, 0x04, 0x02, 0x02, 0x00, 0x03, 0x12, 0x03, 0x0d, 0x17, 0x18, 0x0a, 0x0a, 0x0a, 0x02, 0x04,
    0x03, 0x12, 0x04, 0x10, 0x00, 0x12, 0x01, 0x0a, 0x0a, 0x0a, 0x03, 0x04, 0x03, 0x01, 0x12, 0x03,
    0x10, 0x08, 0x18, 0x0a, 0x0b, 0x0a, 0x04, 0x04, 0x03, 0x02, 0x00, 0x12, 0x03, 0x11, 0x02, 0x13,
    0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x03, 0x02, 0x00, 0x05, 0x12, 0x03, 0x11, 0x02, 0x08, 0x0a, 0x0c,
    0x0a, 0x05, 0x04, 0x03, 0x02, 0x00, 0x01, 0x12, 0x03, 0x11, 0x09, 0x0e, 0x0a, 0x0c, 0x0a, 0x05,
    0x04, 0x03, 0x02, 0x00, 0x03, 0x12, 0x03, 0x11, 0x11, 0x12, 0x0a, 0x0a, 0x0a, 0x02, 0x04, 0x04,
    0x12, 0x04, 0x13, 0x00, 0x16, 0x01, 0x0a, 0x0a, 0x0a, 0x03, 0x04, 0x04, 0x01, 0x12, 0x03, 0x13,
    0x08, 0x19, 0x0a, 0x0b, 0x0a, 0x04, 0x04, 0x04, 0x02, 0x00, 0x12, 0x03, 0x14, 0x02, 0x18, 0x0a,
    0x0c, 0x0a, 0x05, 0x04, 0x04, 0x02, 0x00, 0x05, 0x12, 0x03, 0x14, 0x02, 0x08, 0x0a, 0x0c, 0x0a,
    0x05, 0x04, 0x04, 0x02, 0x00, 0x01, 0x12, 0x03, 0x14, 0x09, 0x13, 0x0a, 0x0c, 0x0a, 0x05, 0x04,
    0x04, 0x02, 0x00, 0x03, 0x12, 0x03, 0x14, 0x16, 0x17, 0x0a, 0x28, 0x0a, 0x04, 0x04, 0x04, 0x02,
    0x01, 0x12, 0x03, 0x15, 0x02, 0x12, 0x22, 0x1b, 0x20, 0x54, 0x4f, 0x44, 0x4f, 0x3a, 0x20, 0x75,
    0x73, 0x65, 0x20, 0x75, 0x35, 0x63, 0x20, 0x74, 0x72, 0x61, 0x6e, 0x73, 0x61, 0x63, 0x74, 0x69,
    0x6f, 0x6e, 0x0a, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x04, 0x02, 0x01, 0x05, 0x12, 0x03, 0x15, 0x02,
    0x08, 0x0a, 0x0c, 0x0a, 0x05, 0x04, 0x04, 0x02, 0x01, 0x01, 0x12, 0x03, 0x15, 0x09, 0x0d, 0x0a,
    0x0c, 0x0a, 0x05, 0x04, 0x04, 0x02, 0x01, 0x03, 0x12, 0x03, 0x15, 0x10, 0x11, 0x0a, 0x0a, 0x0a,
    0x02, 0x06, 0x00, 0x12, 0x04, 0x18, 0x00, 0x1b, 0x01, 0x0a, 0x0a, 0x0a, 0x03, 0x06, 0x00, 0x01,
    0x12, 0x03, 0x18, 0x08, 0x15, 0x0a, 0x0b, 0x0a, 0x04, 0x06, 0x00, 0x02, 0x00, 0x12, 0x03, 0x19,
    0x02, 0x3b, 0x0a, 0x0c, 0x0a, 0x05, 0x06, 0x00, 0x02, 0x00, 0x01, 0x12, 0x03, 0x19, 0x06, 0x0e,
    0x0a, 0x0c, 0x0a, 0x05, 0x06, 0x00, 0x02, 0x00, 0x02, 0x12, 0x03, 0x19, 0x0f, 0x1e, 0x0a, 0x0c,
    0x0a, 0x05, 0x06, 0x00, 0x02, 0x00, 0x03, 0x12, 0x03, 0x19, 0x29, 0x39, 0x0a, 0x0b, 0x0a, 0x04,
    0x06, 0x00, 0x02, 0x01, 0x12, 0x03, 0x1a, 0x02, 0x3e, 0x0a, 0x0c, 0x0a, 0x05, 0x06, 0x00, 0x02,
    0x01, 0x01, 0x12, 0x03, 0x1a, 0x06, 0x0f, 0x0a, 0x0c, 0x0a, 0x05, 0x06, 0x00, 0x02, 0x01, 0x02,
    0x12, 0x03, 0x1a, 0x10, 0x20, 0x0a, 0x0c, 0x0a, 0x05, 0x06, 0x00, 0x02, 0x01, 0x03, 0x12, 0x03,
    0x1a, 0x2b, 0x3c, 0x62, 0x06, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x33,
];
include!("boros.v1.submit.serde.rs");
include!("boros.v1.submit.tonic.rs");
// @@protoc_insertion_point(module)