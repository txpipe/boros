// @generated
impl serde::Serialize for LockStateRequest {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if !self.queue.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("boros.v1.LockStateRequest", len)?;
        if !self.queue.is_empty() {
            struct_ser.serialize_field("queue", &self.queue)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for LockStateRequest {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "queue",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Queue,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "queue" => Ok(GeneratedField::Queue),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = LockStateRequest;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct boros.v1.LockStateRequest")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<LockStateRequest, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut queue__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Queue => {
                            if queue__.is_some() {
                                return Err(serde::de::Error::duplicate_field("queue"));
                            }
                            queue__ = Some(map_.next_value()?);
                        }
                    }
                }
                Ok(LockStateRequest {
                    queue: queue__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("boros.v1.LockStateRequest", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for LockStateResponse {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if !self.lock_token.is_empty() {
            len += 1;
        }
        if !self.cbor.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("boros.v1.LockStateResponse", len)?;
        if !self.lock_token.is_empty() {
            struct_ser.serialize_field("lockToken", &self.lock_token)?;
        }
        if !self.cbor.is_empty() {
            struct_ser.serialize_field("cbor", &self.cbor)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for LockStateResponse {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "lock_token",
            "lockToken",
            "cbor",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            LockToken,
            Cbor,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "lockToken" | "lock_token" => Ok(GeneratedField::LockToken),
                            "cbor" => Ok(GeneratedField::Cbor),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = LockStateResponse;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct boros.v1.LockStateResponse")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<LockStateResponse, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut lock_token__ = None;
                let mut cbor__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::LockToken => {
                            if lock_token__.is_some() {
                                return Err(serde::de::Error::duplicate_field("lockToken"));
                            }
                            lock_token__ = Some(map_.next_value()?);
                        }
                        GeneratedField::Cbor => {
                            if cbor__.is_some() {
                                return Err(serde::de::Error::duplicate_field("cbor"));
                            }
                            cbor__ = Some(map_.next_value()?);
                        }
                    }
                }
                Ok(LockStateResponse {
                    lock_token: lock_token__.unwrap_or_default(),
                    cbor: cbor__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("boros.v1.LockStateResponse", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for SubmitTxRequest {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if !self.tx.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("boros.v1.SubmitTxRequest", len)?;
        if !self.tx.is_empty() {
            struct_ser.serialize_field("tx", &self.tx)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for SubmitTxRequest {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "tx",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Tx,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "tx" => Ok(GeneratedField::Tx),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = SubmitTxRequest;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct boros.v1.SubmitTxRequest")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<SubmitTxRequest, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut tx__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Tx => {
                            if tx__.is_some() {
                                return Err(serde::de::Error::duplicate_field("tx"));
                            }
                            tx__ = Some(map_.next_value()?);
                        }
                    }
                }
                Ok(SubmitTxRequest {
                    tx: tx__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("boros.v1.SubmitTxRequest", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for SubmitTxResponse {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if !self.r#ref.is_empty() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("boros.v1.SubmitTxResponse", len)?;
        if !self.r#ref.is_empty() {
            struct_ser.serialize_field("ref", &self.r#ref.iter().map(pbjson::private::base64::encode).collect::<Vec<_>>())?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for SubmitTxResponse {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "ref",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Ref,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "ref" => Ok(GeneratedField::Ref),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = SubmitTxResponse;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct boros.v1.SubmitTxResponse")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<SubmitTxResponse, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut r#ref__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Ref => {
                            if r#ref__.is_some() {
                                return Err(serde::de::Error::duplicate_field("ref"));
                            }
                            r#ref__ = 
                                Some(map_.next_value::<Vec<::pbjson::private::BytesDeserialize<_>>>()?
                                    .into_iter().map(|x| x.0).collect())
                            ;
                        }
                    }
                }
                Ok(SubmitTxResponse {
                    r#ref: r#ref__.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("boros.v1.SubmitTxResponse", FIELDS, GeneratedVisitor)
    }
}
impl serde::Serialize for Tx {
    #[allow(deprecated)]
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut len = 0;
        if !self.raw.is_empty() {
            len += 1;
        }
        if self.queue.is_some() {
            len += 1;
        }
        if self.lock_token.is_some() {
            len += 1;
        }
        let mut struct_ser = serializer.serialize_struct("boros.v1.Tx", len)?;
        if !self.raw.is_empty() {
            #[allow(clippy::needless_borrow)]
            struct_ser.serialize_field("raw", pbjson::private::base64::encode(&self.raw).as_str())?;
        }
        if let Some(v) = self.queue.as_ref() {
            struct_ser.serialize_field("queue", v)?;
        }
        if let Some(v) = self.lock_token.as_ref() {
            struct_ser.serialize_field("lockToken", v)?;
        }
        struct_ser.end()
    }
}
impl<'de> serde::Deserialize<'de> for Tx {
    #[allow(deprecated)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const FIELDS: &[&str] = &[
            "raw",
            "queue",
            "lock_token",
            "lockToken",
        ];

        #[allow(clippy::enum_variant_names)]
        enum GeneratedField {
            Raw,
            Queue,
            LockToken,
        }
        impl<'de> serde::Deserialize<'de> for GeneratedField {
            fn deserialize<D>(deserializer: D) -> std::result::Result<GeneratedField, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct GeneratedVisitor;

                impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
                    type Value = GeneratedField;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(formatter, "expected one of: {:?}", &FIELDS)
                    }

                    #[allow(unused_variables)]
                    fn visit_str<E>(self, value: &str) -> std::result::Result<GeneratedField, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "raw" => Ok(GeneratedField::Raw),
                            "queue" => Ok(GeneratedField::Queue),
                            "lockToken" | "lock_token" => Ok(GeneratedField::LockToken),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(GeneratedVisitor)
            }
        }
        struct GeneratedVisitor;
        impl<'de> serde::de::Visitor<'de> for GeneratedVisitor {
            type Value = Tx;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("struct boros.v1.Tx")
            }

            fn visit_map<V>(self, mut map_: V) -> std::result::Result<Tx, V::Error>
                where
                    V: serde::de::MapAccess<'de>,
            {
                let mut raw__ = None;
                let mut queue__ = None;
                let mut lock_token__ = None;
                while let Some(k) = map_.next_key()? {
                    match k {
                        GeneratedField::Raw => {
                            if raw__.is_some() {
                                return Err(serde::de::Error::duplicate_field("raw"));
                            }
                            raw__ = 
                                Some(map_.next_value::<::pbjson::private::BytesDeserialize<_>>()?.0)
                            ;
                        }
                        GeneratedField::Queue => {
                            if queue__.is_some() {
                                return Err(serde::de::Error::duplicate_field("queue"));
                            }
                            queue__ = map_.next_value()?;
                        }
                        GeneratedField::LockToken => {
                            if lock_token__.is_some() {
                                return Err(serde::de::Error::duplicate_field("lockToken"));
                            }
                            lock_token__ = map_.next_value()?;
                        }
                    }
                }
                Ok(Tx {
                    raw: raw__.unwrap_or_default(),
                    queue: queue__,
                    lock_token: lock_token__,
                })
            }
        }
        deserializer.deserialize_struct("boros.v1.Tx", FIELDS, GeneratedVisitor)
    }
}
