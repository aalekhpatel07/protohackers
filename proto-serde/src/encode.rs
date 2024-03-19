use serde::{ser::SerializeSeq, Serialize};
use std::io::Write;
use crate::error::ser::Error;


pub struct Protohackers<W: Write> {
    writer: W
}

pub fn to_writer<T: serde::ser::Serialize, W: Write>(data: &T, writer: W) -> Result<(), Error> {
    let mut ph = Protohackers { writer };
    data.serialize(&mut ph)
}

pub fn to_vec<T: serde::ser::Serialize>(data: &T) -> Result<Vec<u8>, Error> {
    let buffer = vec![];
    let mut ph = Protohackers { writer: buffer };
    data.serialize(&mut ph)?;
    Ok(ph.writer)
}

impl<'a, W> serde::ser::Serializer for &'a mut Protohackers<W> 
where
    W: std::io::Write
{
    type Ok = ();
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeStructVariant = Self;
    type SerializeStruct = Self;
    type SerializeMap = Self;
    type Error = Error;

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(Error::UnsupportedDataType { r#type: "bool".to_string(), context: Some("Not specified in Protohackers".to_string()) })
    }
    fn is_human_readable(&self) -> bool {
        false
    }
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        (v as u8).serialize(self)
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    {
        value.serialize(self)
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        if v.len() > 255 {
            return Err(Error::ValueTooBig { r#type: "str".to_string(), max_value: 255, observed: Some(v.len().try_into()?), _inner: None });
        }
        if !v.is_ascii() {
            return Err(Error::UnsupportedDataType { r#type: "str".to_string(), context: Some("string is not valid ascii".to_string()) });
        }
        self.writer.write_all(&[v.len() as u8])?;
        self.writer.write_all(v.as_bytes())?;

        Ok(())
    }
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.writer.write_all(&[v])?;
        Ok(())
    }
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.writer.write_all(&v.to_be_bytes())?;
        Ok(())
    }
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.writer.write_all(&v.to_be_bytes())?;
        Ok(())
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        let Some(len) = len else { 
            return Err(Error::UnknownLength)
        };
        let len: u32 = len.try_into()?;
        self.writer.write_all(&len.to_be_bytes())?;
        Ok(self)
    }
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        // Tuples also lose all component ordering data.
        self.serialize_map(Some(len))
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_map(Some(len))
    }
    
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.writer.write_all(&v.to_be_bytes())?;
        Ok(())
    }
    
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.writer.write_all(&v.to_be_bytes())?;
        Ok(())
    }
    
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.writer.write_all(&v.to_be_bytes())?;
        Ok(())
    }
    
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.writer.write_all(&v.to_be_bytes())?;
        Ok(())
    }
    
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.writer.write_all(&v.to_be_bytes())?;
        Ok(())
    }
    
    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(Error::UnsupportedDataType { r#type: "f32".to_string(), context: Some("Not specified in Protohackers".to_string()) })
    }
    
    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Error::UnsupportedDataType { r#type: "f64".to_string(), context: Some("Not specified in Protohackers".to_string()) })
    }
    
    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        let mut seq = self.serialize_seq(Some(v.len()))?;
        for b in v {
            seq.serialize_element(b)?;
        }
        seq.end()
    }
    
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
    
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
    {
        value.serialize(self)
    }
    
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
    
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }
    
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> 
    {
        // FIXME: What do unit variants really represent and how do they fit in network messages
        // for Protohackers?
        Err(Error::UnsupportedDataType { r#type: "UnitVariant".to_string(), context: Some("There is no meaningful way to serialize a UnitVariant for Protohackers.".to_string()) })
    }
    
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    {
        // FIXME: What do newtype variants really represent and how do they fit in network messages
        // for Protohackers? Should it be safe to forward to the wrapped type?
        value.serialize(self)
    }
    
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {

        // FIXME: What do tuple variants really represent and how do they fit in network messages
        // for Protohackers? Should it be safe to forward to the wrapped tuple?
        self.serialize_tuple(len)
    }
    
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        if len.is_none() { return Err(Error::UnknownLength)};

        // No need to preserve names when we serialize maps/structs.
        Ok(self)
    }
    
    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.serialize_struct(name, len)
    }
}


impl<'a, W> serde::ser::SerializeSeq for &'a mut Protohackers<W> 
where 
    W: Write
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error>
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}


impl<'a, W> serde::ser::SerializeTuple for &'a mut Protohackers<W> 
where 
    W: Write
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error>
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}


impl<'a, W> serde::ser::SerializeTupleStruct for &'a mut Protohackers<W> 
where 
    W: Write
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error>
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}


impl<'a, W> serde::ser::SerializeStruct for &'a mut Protohackers<W> 
where 
    W: Write
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + serde::Serialize>(
        &mut self,
        _key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    {
        // serialization happens without names.
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}


impl<'a, W> serde::ser::SerializeMap for &'a mut Protohackers<W> 
where 
    W: Write
{
    type Ok = ();
    type Error = Error;

    fn serialize_entry<K: ?Sized + serde::Serialize, V: ?Sized + serde::Serialize>(
        &mut self,
        _key: &K,
        value: &V,
    ) -> Result<(), Self::Error>
    {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_key<T: ?Sized + serde::Serialize>(&mut self, _key: &T) -> Result<(), Self::Error>
    {
        // serialization happens without names.
        Ok(())
    }

    fn serialize_value<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error>
    {
        value.serialize(&mut **self)
    }
}


impl<'a, W> serde::ser::SerializeTupleVariant for &'a mut Protohackers<W> 
where 
    W: Write
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error>
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}


impl<'a, W> serde::ser::SerializeStructVariant for &'a mut Protohackers<W> 
where 
    W: Write
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, _key: &'static str, value: &T) -> Result<(), Self::Error>
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}