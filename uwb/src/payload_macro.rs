use crate::config::UwbConfig;
use crate::error::*;
use crate::frame::{MacAddress, MacFrame, MacFrameControl};
use crate::frame_builder::{MacFrameBuilder, MacFrameFactory};
use crate::time::UWBTimestamp;

#[macro_export]
macro_rules! define_payload {
    (
        $(
            $variant: ident = $val: expr $(, { $($field_name: ident : $field_typ: ty),* $(,)? })?
        ),* $(,)?
    ) =>{
        // Generate the main MessageType enum
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        pub enum MessageType {
            $(
                $variant = $val,
            )*
        }

        // Generate an enum to wrap the active builders at runtime
        pub enum TypedBuilder {
            $(
                $variant($crate::frame_builder::MacFrameBuilder<$variant, {UwbConfig::MAX_STANDARD_FRAME_SIZE}>),
            )*
        }

        // Generate UwBPayload Enum with associated parameters
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum UwbPayload {
            $(
                $variant $( { $( $field_name : $field_typ ),* } )?,
            )*
        }

        // Implementation of the UwBPayload enum
        impl UwbPayload {
            pub fn get_message_type(&self) -> MessageType {
                match self {
                    $(
                        UwbPayload::$variant $( { $($field_name: _),* } )? => MessageType::$variant,
                    )*
                }
            }

            pub fn to_bytes<const N: usize>(&self) -> Result<heapless::Vec<u8, N>, UwbError> {
                let mut buf = heapless::Vec::new();

                // 1. MessageType
                buf.push(self.get_message_type() as u8)
                    .map_err(|_| UwbError::Protocol(ProtocolError::BufferOverflow))?;

                // 2. Payload
                match self {
                    $(
                        UwbPayload::$variant $({ $($field_name),* })? => {
                            $(
                                $(
                                    $crate::UwbSerialize::serialize($field_name, &mut buf)?;
                                )*
                            )?
                        }
                    )*
                }
                Ok(buf)
            }

            pub fn from_bytes(bytes: &[u8]) -> Result<Self, UwbError> {
                if bytes.is_empty() {
                    return Err(UwbError::Protocol(ProtocolError::InvalidFrameForamt));
                }

                let mut offset = 0;
                let message_type_id = bytes[offset];
                offset += 1;

                match message_type_id {
                    $(
                        $val => {
                            $(
                                $(
                                    let $field_name = <$field_typ as $crate::UwbSerialize>::deserialize(bytes, &mut offset)?;
                                )*
                            )?

                            Ok(UwbPayload::$variant $( { $($field_name),* } )?)
                        }
                    )*

                    _ => Err(UwbError::Protocol(ProtocolError::InvalidFrameType)),
                }
            }
        }


        // Generate the compile-time dummy structs for the Builder pattern
        $(
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub struct $variant;


        )*

        // Automatically generate the Builder initialization methods
        paste::paste! {
            impl $crate::MacFrameFactory {
                $(
                    pub fn [<new_ $variant:lower>]() -> $crate::frame_builder::MacFrameBuilder<$variant, {UwbConfig::MAX_STANDARD_FRAME_SIZE}> {
                        $crate::frame_builder::MacFrameBuilder {
                            state: $variant,
                            sequence_number: 0,
                            dest_pan_id: 0xAAAA,
                            dest_addr: MacAddress::None,
                            src_addr: MacAddress::None,
                            frame_control: MacFrameControl::default(),
                        }
                    }
                )*

                pub fn new(msg_type: MessageType) -> TypedBuilder {
                    match msg_type {
                        $(
                            MessageType::$variant => TypedBuilder::$variant(Self::[<new_ $variant:lower>]()),
                        )*
                    }
                }
            }
        }

        $(
            $crate::define_payload! {@build $variant $( { $($field_name : $field_typ),* } )?}
        )*


    };

    (
        @build $variant:ident { $($field_name:ident : $field_typ:ty),*}
    ) =>{
         impl<const N: usize> $crate::frame_builder::MacFrameBuilder<$variant, N> {
                pub fn build(
                    self,
                    payload: UwbPayload
                ) -> Result<MacFrame<N>, UwbError> {
                    if !matches!(payload, UwbPayload::$variant { .. }) {
                        return Err(UwbError::Protocol(ProtocolError::InvalidFrameForamt));
                    }

                    let serialized_payload = payload.to_bytes()?;
                    let mac_frame = MacFrame {
                        sequence_number: self.sequence_number,
                        dest_pan_id: self.dest_pan_id,
                        dest_addr: self.dest_addr,
                        src_addr: self.src_addr,
                        frame_control: self.frame_control,
                        payload: serialized_payload,
                    };

                    Ok(mac_frame)
                }

                pub fn build_with_params(
                    self,
                    $( $field_name: $field_typ),*
                ) -> Result<MacFrame<N>, UwbError> {
                    let payload = UwbPayload::$variant {
                        $( $field_name ),*
                    };
                    self.build(payload)
                }
        }

    };

    (
        @build $variant:ident
    ) =>{
         impl<const N: usize> $crate::frame_builder::MacFrameBuilder<$variant, N> {
                pub fn build(
                    self,
                ) -> Result<MacFrame<N>, UwbError> {
                    let serialized_payload = UwbPayload::$variant.to_bytes()?;
                    let mac_frame = MacFrame {
                        sequence_number: self.sequence_number,
                        dest_pan_id: self.dest_pan_id,
                        dest_addr: self.dest_addr,
                        src_addr: self.src_addr,
                        frame_control: self.frame_control,
                        payload: serialized_payload,
                    };

                    Ok(mac_frame)

                }
            }

    };
}
