use crate::memory::Memory;

pub const OBJECT_POOL_BASE: u16 = 0x03BD;
pub const OBJECT_STRIDE: u16 = 0x003F;
pub const OBJECT_COUNT: usize = 0x3C;
pub const ACTIVE_LIST: u16 = 0x12A8;
pub const FREE_LIST: u16 = 0x12AA;
pub const CURRENT_OBJECT: u16 = 0x1651;
pub const SELECTED_OBJECT: u16 = 0xCF1F;
pub const PLAYER_ONE: u16 = 0x12C3;
pub const PLAYER_TWO: u16 = 0x12C5;

/// Retail's separate 19-byte map/radar record pool. Map opcode `$90` uses
/// these records through the free/active heads at `$1283/$1285`; they are not
/// gameplay objects and must never consume the 60-entry `$03BD` object pool.
pub const MAP_AUX_POOL_BASE: u16 = 0x342E;
pub const MAP_AUX_STRIDE: u16 = 0x0019;
pub const MAP_AUX_COUNT: usize = 0x0200;
pub const MAP_AUX_FREE_LIST: u16 = 0x1283;
pub const MAP_AUX_ACTIVE_LIST: u16 = 0x1285;

pub const FIELD_NEXT: u16 = 0x00;
pub const FIELD_PREV: u16 = 0x02;
pub const FIELD_SHAPE: u16 = 0x04;
pub const FIELD_X: u16 = 0x0C;
pub const FIELD_Y: u16 = 0x0E;
pub const FIELD_Z: u16 = 0x10;
pub const FIELD_ROT_X: u16 = 0x12;
pub const FIELD_ROT_Y: u16 = 0x14;
pub const FIELD_ROT_Z: u16 = 0x16;
pub const FIELD_STRATEGY: u16 = 0x19;
pub const FIELD_PATH: u16 = 0x2B;

/// Base and size of the retail variable-size WRAM heap initialized by
/// `$7F:1737`.  Heap pointers are offsets from this base, not CPU addresses.
pub const AUX_HEAP_BASE: u16 = 0x6A61;
pub const AUX_HEAP_FIRST_BLOCK: u16 = 0x0002;
pub const AUX_HEAP_SIZE: u16 = 0x47FE;

const AUX_CHAIN_HEAD: u16 = 0x1CDC;
const AUX_TYPE_TABLE: u16 = 0x1CEC;

pub fn object_address(index: usize) -> u16 {
    OBJECT_POOL_BASE.wrapping_add(OBJECT_STRIDE.wrapping_mul(index as u16))
}

pub fn object_index(address: u16) -> Option<usize> {
    let delta = address.checked_sub(OBJECT_POOL_BASE)?;
    if delta % OBJECT_STRIDE != 0 {
        return None;
    }
    let index = usize::from(delta / OBJECT_STRIDE);
    (index < OBJECT_COUNT).then_some(index)
}

pub fn initialize_pool(memory: &mut Memory) {
    memory.write_word(ACTIVE_LIST, 0);
    memory.write_word(CURRENT_OBJECT, 0);
    for index in 0..OBJECT_COUNT {
        let address = object_address(index);
        for byte in 0..OBJECT_STRIDE {
            memory.write_byte(address.wrapping_add(byte), 0);
        }
        let next = if index + 1 < OBJECT_COUNT {
            object_address(index + 1)
        } else {
            0
        };
        memory.write_word(address + FIELD_NEXT, next);
    }
    memory.write_word(FREE_LIST, OBJECT_POOL_BASE);
    initialize_map_auxiliary_pool(memory);
    initialize_auxiliary_heap(memory);
}

/// Retail `$0D:D5B8`: format all 512 map/radar records as a forward free
/// list and clear the active-list head.
pub fn initialize_map_auxiliary_pool(memory: &mut Memory) {
    memory.write_word(MAP_AUX_ACTIVE_LIST, 0);
    memory.write_word(MAP_AUX_FREE_LIST, MAP_AUX_POOL_BASE);
    for index in 0..MAP_AUX_COUNT {
        let record = MAP_AUX_POOL_BASE.wrapping_add(MAP_AUX_STRIDE * index as u16);
        let next = if index + 1 < MAP_AUX_COUNT {
            record.wrapping_add(MAP_AUX_STRIDE)
        } else {
            0
        };
        memory.write_word(record, next);
    }
}

/// Pop a retail map/radar record and insert it immediately after the active
/// head, matching the common allocator embedded in map opcode `$90`.
pub fn allocate_map_auxiliary(memory: &mut Memory) -> Option<u16> {
    let record = memory.read_word(MAP_AUX_FREE_LIST);
    if record == 0 {
        return None;
    }
    let next_free = memory.read_word(record);
    memory.write_word(MAP_AUX_FREE_LIST, next_free);

    let head = memory.read_word(MAP_AUX_ACTIVE_LIST);
    if head == 0 {
        memory.write_word(record, 0);
        memory.write_word(record + 2, 0);
        memory.write_word(MAP_AUX_ACTIVE_LIST, record);
    } else {
        let successor = memory.read_word(head);
        memory.write_word(record, successor);
        memory.write_word(head, record);
        memory.write_word(record + 2, head);
        if successor != 0 {
            memory.write_word(successor + 2, record);
        }
    }
    Some(record)
}

pub fn active_map_auxiliaries(memory: &Memory) -> Vec<u16> {
    let mut records = Vec::new();
    let mut record = memory.read_word(MAP_AUX_ACTIVE_LIST);
    while record != 0 && records.len() < MAP_AUX_COUNT {
        records.push(record);
        record = memory.read_word(record);
    }
    records
}

/// Retail `$7F:1737`: make the whole `$6A63..$B260` area one free block.
pub fn initialize_auxiliary_heap(memory: &mut Memory) {
    memory.write_word(AUX_HEAP_BASE, AUX_HEAP_FIRST_BLOCK);
    heap_write_word(memory, AUX_HEAP_FIRST_BLOCK, 0, 0);
    heap_write_word(memory, AUX_HEAP_FIRST_BLOCK, 2, 0);
    heap_write_word(memory, AUX_HEAP_FIRST_BLOCK, 4, AUX_HEAP_SIZE);
}

fn heap_address(offset: u16, field: u16) -> u16 {
    AUX_HEAP_BASE.wrapping_add(offset).wrapping_add(field)
}

fn heap_read_word(memory: &Memory, offset: u16, field: u16) -> u16 {
    memory.read_word(heap_address(offset, field))
}

fn heap_write_word(memory: &mut Memory, offset: u16, field: u16, value: u16) {
    memory.write_word(heap_address(offset, field), value);
}

fn heap_remove_free_block(memory: &mut Memory, block: u16) {
    let next = heap_read_word(memory, block, 0);
    let previous = heap_read_word(memory, block, 2);
    if previous == 0 {
        memory.write_word(AUX_HEAP_BASE, next);
    } else {
        heap_write_word(memory, previous, 0, next);
    }
    if next != 0 {
        heap_write_word(memory, next, 2, previous);
    }
}

/// Retail `$7F:18A7`. `requested` is the number of payload bytes before the
/// allocator's own two-byte block header. The returned offset points just
/// beyond that header.
pub fn heap_allocate(memory: &mut Memory, requested: u16) -> Option<u16> {
    // The retail allocator uses `$B261` for both its requested-size scratch
    // and its return value.  A surprising amount of surrounding engine code
    // leaves these work words observable, so preserve them as part of the
    // routine's state contract rather than treating them as local variables.
    memory.write_word(0xB261, requested);
    let mut total = requested.wrapping_add(1) & 0xFFFE;
    if total == 0 {
        memory.write_word(0xB261, 0);
        return None;
    }
    total = total.wrapping_add(2).max(6);

    let mut block = memory.read_word(AUX_HEAP_BASE);
    while block != 0 && heap_read_word(memory, block, 4) < total {
        block = heap_read_word(memory, block, 0);
    }
    if block == 0 {
        memory.write_word(0xB261, 0);
        return None;
    }

    let block_size = heap_read_word(memory, block, 4);
    let remainder = block_size.wrapping_sub(total);
    let header = if remainder > 6 {
        heap_write_word(memory, block, 4, remainder);
        block.wrapping_add(remainder)
    } else {
        heap_remove_free_block(memory, block);
        total = block_size;
        block
    };
    // Allocated blocks store their total size in their first word. Free
    // blocks use that word for the next pointer and keep size at +4.
    heap_write_word(memory, header, 0, total);
    let payload = header.wrapping_add(2);
    memory.write_word(0xB261, payload);
    Some(payload)
}

/// Retail `$7F:1777`. `payload` is the offset returned by `heap_allocate`.
pub fn heap_free(memory: &mut Memory, payload: u16) -> bool {
    let Some(header) = payload.checked_sub(2) else {
        return false;
    };
    if header < AUX_HEAP_FIRST_BLOCK || header >= AUX_HEAP_FIRST_BLOCK + AUX_HEAP_SIZE {
        return false;
    }
    let size = heap_read_word(memory, header, 0);
    if size < 6 || header.wrapping_add(size) > AUX_HEAP_FIRST_BLOCK + AUX_HEAP_SIZE {
        return false;
    }

    let end = header.wrapping_add(size);
    // `$7F:1777` materializes these three words before it searches the free
    // list.  They overlap at byte boundaries exactly as shown here and remain
    // live after return (`$B263=end`, `$B265=size`).
    memory.write_word(0xB261, header);
    memory.write_word(0xB263, end);
    memory.write_word(0xB265, size);
    let old_head = memory.read_word(AUX_HEAP_BASE);
    let mut previous_adjacent = 0;
    let mut next_adjacent = 0;
    let mut block = memory.read_word(AUX_HEAP_BASE);
    while block != 0 {
        let block_size = heap_read_word(memory, block, 4);
        if block.wrapping_add(block_size) == header {
            previous_adjacent = block;
        }
        if block == end {
            next_adjacent = block;
        }
        block = heap_read_word(memory, block, 0);
    }

    if previous_adjacent != 0 {
        let mut merged_size = heap_read_word(memory, previous_adjacent, 4).wrapping_add(size);
        if next_adjacent != 0 {
            merged_size = merged_size.wrapping_add(heap_read_word(memory, next_adjacent, 4));
            heap_remove_free_block(memory, next_adjacent);
        }
        heap_write_word(memory, previous_adjacent, 4, merged_size);
        // The merge helper's unsuccessful adjacency scan exits with the null
        // next link in A; `$7F:1777` then exposes it through `$B261`.
        memory.write_word(0xB261, 0);
        return true;
    }

    if next_adjacent != 0 {
        let next = heap_read_word(memory, next_adjacent, 0);
        let previous = heap_read_word(memory, next_adjacent, 2);
        let merged_size = size.wrapping_add(heap_read_word(memory, next_adjacent, 4));
        heap_write_word(memory, header, 0, next);
        heap_write_word(memory, header, 2, previous);
        heap_write_word(memory, header, 4, merged_size);
        if previous == 0 {
            memory.write_word(AUX_HEAP_BASE, header);
        } else {
            heap_write_word(memory, previous, 0, header);
        }
        if next != 0 {
            heap_write_word(memory, next, 2, header);
        }
        memory.write_word(0xB261, 0);
        return true;
    }

    // With no adjacent block retail pushes the new free block at the head.
    heap_write_word(memory, header, 0, old_head);
    heap_write_word(memory, header, 2, 0);
    heap_write_word(memory, header, 4, size);
    if old_head != 0 {
        heap_write_word(memory, old_head, 2, header);
    }
    memory.write_word(AUX_HEAP_BASE, header);
    memory.write_word(0xB261, if old_head == 0 { size } else { old_head });
    true
}

/// Retail `$7F:194E`: allocate an object-owned block. The first two payload
/// bytes form the object's allocation chain, so callers receive the offset
/// two bytes beyond the raw heap payload.
pub fn allocate_auxiliary(memory: &mut Memory, object: u16, size: u16) -> Option<u16> {
    object_index(object)?;
    let raw = heap_allocate(memory, size.wrapping_add(2))?;
    let old_head = memory.read_word(object.wrapping_add(AUX_CHAIN_HEAD));
    heap_write_word(memory, raw, 0, old_head);
    memory.write_word(object.wrapping_add(AUX_CHAIN_HEAD), raw);
    Some(raw.wrapping_add(2))
}

/// Retail `$7F:196B`: unlink and free one object-owned allocation.
pub fn free_auxiliary(memory: &mut Memory, object: u16, public_offset: u16) -> bool {
    if object_index(object).is_none() {
        return false;
    }
    let Some(raw) = public_offset.checked_sub(2) else {
        return false;
    };
    let mut previous = 0;
    let mut block = memory.read_word(object.wrapping_add(AUX_CHAIN_HEAD));
    while block != 0 && block != raw {
        previous = block;
        block = heap_read_word(memory, block, 0);
    }
    if block == 0 {
        return false;
    }
    let next = heap_read_word(memory, block, 0);
    if previous == 0 {
        memory.write_word(object.wrapping_add(AUX_CHAIN_HEAD), next);
    } else {
        heap_write_word(memory, previous, 0, next);
    }
    heap_free(memory, block)
}

/// Retail `$7F:1B00`: grow an object-owned block and preserve the same byte
/// count used by the original routine.
pub fn resize_auxiliary(
    memory: &mut Memory,
    object: u16,
    public_offset: u16,
    new_size: u16,
) -> Option<u16> {
    if public_offset == 0 {
        return allocate_auxiliary(memory, object, new_size);
    }
    let new_offset = allocate_auxiliary(memory, object, new_size)?;
    let allocation_size = heap_read_word(memory, public_offset.wrapping_sub(4), 0);
    let copy_size = allocation_size.min(new_size);
    let bytes: Vec<u8> = (0..copy_size)
        .map(|index| memory.read_byte(heap_address(public_offset, index)))
        .collect();
    for (index, byte) in bytes.into_iter().enumerate() {
        memory.write_byte(heap_address(new_offset, index as u16), byte);
    }
    free_auxiliary(memory, object, public_offset);
    // `$7F:1B00` pops the newly allocated public pointer into both its return
    // value and the persistent resize scratch word immediately before RTL.
    memory.write_word(0xCF2B, new_offset);
    Some(new_offset)
}

/// Retail `$7F:19A7`: release every object-owned allocation and clear the
/// extension pointers coupled to that list.
pub fn free_all_auxiliary(memory: &mut Memory, object: u16) {
    if object_index(object).is_none() {
        return;
    }
    let mut raw = memory.read_word(object.wrapping_add(AUX_CHAIN_HEAD));
    while raw != 0 {
        let next = heap_read_word(memory, raw, 0);
        heap_free(memory, raw);
        raw = next;
    }
    for offset in [AUX_CHAIN_HEAD, AUX_TYPE_TABLE, 0x1CDE, 0x1CE0] {
        memory.write_word(object.wrapping_add(offset), 0);
    }
}

/// Retail `$7F:233B`: return the offset of a four-byte type-table entry.
pub fn find_auxiliary_type(memory: &Memory, object: u16, kind: u8) -> Option<u16> {
    object_index(object)?;
    let base = memory.read_word(object.wrapping_add(AUX_TYPE_TABLE));
    if base == 0 {
        return None;
    }
    let count = memory.read_byte(heap_address(base, 0));
    (0..count).find_map(|index| {
        let entry = base.wrapping_add(1 + u16::from(index) * 4);
        (memory.read_byte(heap_address(entry, 0)) == kind).then_some(entry)
    })
}

/// Retail `$7F:2360`: find a type entry or append one, growing its backing
/// object-owned block when needed.
pub fn get_or_create_auxiliary_type(memory: &mut Memory, object: u16, kind: u8) -> Option<u16> {
    // Retail `$7F:2360` saves A here before either the lookup or append path.
    memory.write_byte(0xCF2A, kind);
    if let Some(entry) = find_auxiliary_type(memory, object, kind) {
        return Some(entry);
    }
    object_index(object)?;
    let old_base = memory.read_word(object.wrapping_add(AUX_TYPE_TABLE));
    let base = if old_base == 0 {
        let base = allocate_auxiliary(memory, object, 5)?;
        memory.write_byte(heap_address(base, 0), 0);
        base
    } else {
        let count = memory.read_byte(heap_address(old_base, 0));
        let size = u16::from(count.wrapping_add(1))
            .wrapping_mul(4)
            .wrapping_add(1);
        resize_auxiliary(memory, object, old_base, size)?
    };
    memory.write_word(object.wrapping_add(AUX_TYPE_TABLE), base);
    let count = memory.read_byte(heap_address(base, 0));
    memory.write_byte(heap_address(base, 0), count.wrapping_add(1));
    let entry = base.wrapping_add(1 + u16::from(count) * 4);
    memory.write_byte(heap_address(entry, 0), kind);
    Some(entry)
}

pub fn read_auxiliary_byte(memory: &Memory, offset: u16) -> u8 {
    memory.read_byte(heap_address(offset, 0))
}

pub fn write_auxiliary_byte(memory: &mut Memory, offset: u16, value: u8) {
    memory.write_byte(heap_address(offset, 0), value);
}

pub fn read_auxiliary_word(memory: &Memory, offset: u16) -> u16 {
    memory.read_word(heap_address(offset, 0))
}

pub fn write_auxiliary_word(memory: &mut Memory, offset: u16, value: u16) {
    memory.write_word(heap_address(offset, 0), value);
}

/// Push one first-word value onto retail's object-local four-byte path stack.
///
/// `$7F:1A0F` stores the stack in the object-owned allocation referenced by
/// parallel field `$1CDE+X`. Byte zero is the entry count; entries begin at
/// byte one and are four bytes wide. The second word is the retail `$B26B`
/// scratch companion, which most path handlers deliberately leave unchanged.
pub fn push_path_stack(memory: &mut Memory, object: u16, value: u16) -> bool {
    if object_index(object).is_none() {
        return false;
    }

    let mut base = memory.read_word(object.wrapping_add(0x1CDE));
    let count = if base == 0 {
        let Some(allocation) = allocate_auxiliary(memory, object, 0x21) else {
            return false;
        };
        base = allocation;
        memory.write_word(object.wrapping_add(0x1CDE), base);
        1u8
    } else {
        let count = read_auxiliary_byte(memory, base).wrapping_add(1);
        if count & 7 == 0 {
            let new_size = u16::from(count.wrapping_add(8))
                .wrapping_mul(4)
                .wrapping_add(1);
            let Some(allocation) = resize_auxiliary(memory, object, base, new_size) else {
                return false;
            };
            base = allocation;
            memory.write_word(object.wrapping_add(0x1CDE), base);
        }
        count
    };

    write_auxiliary_byte(memory, base, count);
    let entry = base
        .wrapping_add(1)
        .wrapping_add(u16::from(count.wrapping_sub(1)).wrapping_mul(4));
    write_auxiliary_word(memory, entry, value);
    write_auxiliary_word(memory, entry.wrapping_add(2), memory.read_word(0xB26B));
    memory.write_word(0xB269, value);
    true
}

/// Pop the first word from retail's object-local path stack (`$7F:1AC1`).
pub fn pop_path_stack(memory: &mut Memory, object: u16) -> Option<u16> {
    object_index(object)?;
    let base = memory.read_word(object.wrapping_add(0x1CDE));
    if base == 0 {
        return None;
    }
    let count = read_auxiliary_byte(memory, base);
    if count == 0 {
        return None;
    }
    let entry = base
        .wrapping_add(1)
        .wrapping_add(u16::from(count.wrapping_sub(1)).wrapping_mul(4));
    let value = read_auxiliary_word(memory, entry);
    let companion = read_auxiliary_word(memory, entry.wrapping_add(2));
    write_auxiliary_byte(memory, base, count.wrapping_sub(1));
    memory.write_word(0xB269, value);
    memory.write_word(0xB26B, companion);
    Some(value)
}

/// Retail `l_add`: pop the free head and insert it after `after`, or at the
/// active-list head when `after` is zero.
pub fn allocate(memory: &mut Memory, after: u16) -> Option<u16> {
    let new_object = memory.read_word(FREE_LIST);
    object_index(new_object)?;
    let next_free = memory.read_word(new_object + FIELD_NEXT);
    memory.write_word(FREE_LIST, next_free);
    for byte in 0..OBJECT_STRIDE {
        memory.write_byte(new_object.wrapping_add(byte), 0);
    }
    // Retail `$7F:29BC` formats both the 59-byte in-record payload (`+$04`
    // through `+$3E`) and the 63 parallel bytes beginning at `$1CC1+X`.
    for byte in 0..OBJECT_STRIDE {
        memory.write_byte(new_object.wrapping_add(0x1CC1).wrapping_add(byte), 0);
    }
    memory.write_byte(new_object + 0x08, 0x10);
    memory.write_byte(new_object + 0x09, 0x08);
    memory.write_byte(new_object + 0x31, 0x04);
    memory.write_byte(new_object + 0x22, 0x04);
    if memory.read_word(0x1B84) & 0x0002 != 0 {
        memory.write_byte(new_object + 0x26, 0x08);
    }
    memory.write_byte(new_object.wrapping_add(0x1CF0), memory.read_byte(0x190E));

    let after = object_index(after).map(|_| after).unwrap_or(0);
    if after == 0 {
        let old_head = memory.read_word(ACTIVE_LIST);
        memory.write_word(new_object + FIELD_NEXT, old_head);
        memory.write_word(new_object + FIELD_PREV, 0);
        if object_index(old_head).is_some() {
            memory.write_word(old_head + FIELD_PREV, new_object);
        }
        memory.write_word(ACTIVE_LIST, new_object);
    } else {
        let next = memory.read_word(after + FIELD_NEXT);
        memory.write_word(new_object + FIELD_NEXT, next);
        memory.write_word(new_object + FIELD_PREV, after);
        memory.write_word(after + FIELD_NEXT, new_object);
        if object_index(next).is_some() {
            memory.write_word(next + FIELD_PREV, new_object);
        }
    }
    memory.write_word(CURRENT_OBJECT, new_object);
    Some(new_object)
}

pub fn free(memory: &mut Memory, object: u16) -> bool {
    if object_index(object).is_none() {
        return false;
    }
    let next = memory.read_word(object + FIELD_NEXT);
    let previous = memory.read_word(object + FIELD_PREV);
    if object_index(previous).is_some() {
        memory.write_word(previous + FIELD_NEXT, next);
    } else {
        memory.write_word(ACTIVE_LIST, next);
    }
    if object_index(next).is_some() {
        memory.write_word(next + FIELD_PREV, previous);
    }
    let free_head = memory.read_word(FREE_LIST);
    memory.write_word(object + FIELD_NEXT, free_head);
    memory.write_word(object + FIELD_PREV, 0);
    memory.write_word(FREE_LIST, object);
    if memory.read_word(CURRENT_OBJECT) == object {
        memory.write_word(CURRENT_OBJECT, 0);
    }
    true
}

pub fn active_objects(memory: &Memory) -> Vec<u16> {
    let mut result = Vec::new();
    let mut object = memory.read_word(ACTIVE_LIST);
    while object_index(object).is_some() && result.len() < OBJECT_COUNT {
        result.push(object);
        object = memory.read_word(object + FIELD_NEXT);
    }
    result
}
